use crate::{UpdateError, platform::Platform};
use semver::Version;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

const GUID_TEXT_BYTES: usize = 36;
const GUID_HYPHEN_OFFSETS: [usize; 4] = [8, 13, 18, 23];
const FILE_COMPONENT_ATTRIBUTES: &str = "256";
const REGISTRY_COMPONENT_ATTRIBUTES: &str = "260";
const MAX_CABINET_NAME_BYTES: usize = 128;

pub(super) const UPGRADE_CODE: &str = "f78632a2-14b8-5f28-b29d-760bded29a9a";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Report {
    template: String,
    properties: Vec<Property>,
    pub(super) files: Vec<File>,
    components: Vec<Component>,
    upgrades: Vec<Upgrade>,
    custom_actions: Vec<serde_json::Value>,
    storages: Vec<serde_json::Value>,
    media: Vec<Media>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Property {
    name: String,
    value: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct File {
    pub(super) id: String,
    component: String,
    pub(super) name: String,
    pub(super) size: String,
    sequence: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Component {
    id: String,
    guid: String,
    directory: String,
    attributes: String,
    keypath: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Upgrade {
    code: String,
    action: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Media {
    disk: String,
    last_sequence: String,
    cabinet: String,
}

pub(super) fn guid(value: &str) -> bool {
    let value = if value.starts_with('{') {
        let Some(inner) = value
            .strip_prefix('{')
            .and_then(|value| value.strip_suffix('}'))
        else {
            return false;
        };
        inner
    } else {
        value
    };
    value.len() == GUID_TEXT_BYTES
        && value.bytes().enumerate().all(|(index, byte)| {
            if GUID_HYPHEN_OFFSETS.contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

fn same_guid(left: &str, right: &str) -> bool {
    guid(left)
        && left
            .trim_matches(['{', '}'])
            .eq_ignore_ascii_case(right.trim_matches(['{', '}']))
}

impl Report {
    pub(super) fn validate(
        &self,
        version: &Version,
        platform: Platform,
    ) -> Result<&str, UpdateError> {
        let architecture = match platform {
            Platform::WindowsX64 => "x64",
            Platform::WindowsArm64 => "arm64",
            _ => return Err(UpdateError::Archive),
        };
        let properties: BTreeMap<_, _> = self
            .properties
            .iter()
            .map(|item| (item.name.as_str(), item.value.as_str()))
            .collect();
        if properties.len() != self.properties.len()
            || !self
                .template
                .split(';')
                .next()
                .is_some_and(|value| value.eq_ignore_ascii_case(architecture))
            || !self.custom_actions.is_empty()
            || !self.storages.is_empty()
            || !version.pre.is_empty()
            || !version.build.is_empty()
            || properties.get("ProductName") != Some(&"TableCraft")
            || properties.get("Manufacturer") != Some(&"Replicraft")
            || properties.get("ProductVersion").copied() != Some(version.to_string().as_str())
            || properties.get("ALLUSERS") != Some(&"1")
            || !properties
                .get("UpgradeCode")
                .is_some_and(|value| same_guid(value, UPGRADE_CODE))
        {
            return Err(UpdateError::Archive);
        }
        if self.media.len() != 1
            || self.media[0].disk != "1"
            || self.media[0].last_sequence != "4"
            || !self.media[0].cabinet.starts_with('#')
            || self.media[0].cabinet.len() > MAX_CABINET_NAME_BYTES
            || !self.media[0].cabinet[1..]
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
            || self.media[0].cabinet.len() < 2
        {
            return Err(UpdateError::Archive);
        }
        let code = properties
            .get("ProductCode")
            .copied()
            .ok_or(UpdateError::Archive)?;
        if !guid(code) {
            return Err(UpdateError::Archive);
        }
        let expected = [
            (
                "TableCraftDesktop",
                "DesktopApplication",
                "tablecraft-desktop.exe",
            ),
            ("ProjectLicenseFile", "ProjectLicense", "LICENSE"),
            (
                "ThirdPartyNoticesFile",
                "ThirdPartyNotices",
                "ThirdPartyLicenses.txt",
            ),
            ("ReleaseManifestFile", "ReleaseManifest", "RELEASE.json"),
        ];
        if self.files.len() != expected.len() || self.components.len() != expected.len() + 1 {
            return Err(UpdateError::Archive);
        }
        let total = self.files.iter().try_fold(0_u64, |total, file| {
            total.checked_add(file.size.parse::<u64>().ok()?)
        });
        if total.is_none_or(|total| total > super::installed::MAX_PAYLOAD_BYTES) {
            return Err(UpdateError::AssetTooLarge);
        }
        let sequences: BTreeSet<_> = self
            .files
            .iter()
            .map(|file| file.sequence.as_str())
            .collect();
        let component_guids: BTreeSet<_> = self
            .components
            .iter()
            .map(|component| component.guid.to_ascii_lowercase())
            .collect();
        if sequences != BTreeSet::from(["1", "2", "3", "4"]) || component_guids.len() != 5 {
            return Err(UpdateError::Archive);
        }
        for (id, owner, name) in expected {
            let files: Vec<_> = self.files.iter().filter(|file| file.id == id).collect();
            if files.len() != 1
                || files[0].component != owner
                || files[0].name.split('|').next_back() != Some(name)
                || files[0]
                    .size
                    .parse::<u64>()
                    .ok()
                    .filter(|size| *size > 0 && *size <= super::installed::MAX_PAYLOAD_BYTES)
                    .is_none()
            {
                return Err(UpdateError::Archive);
            }
            self.component(owner, id, FILE_COMPONENT_ATTRIBUTES)?;
        }
        self.component(
            "ApplicationRegistration",
            "RegistrationValue00",
            REGISTRY_COMPONENT_ATTRIBUTES,
        )?;
        let actions: BTreeSet<_> = self
            .upgrades
            .iter()
            .map(|item| item.action.as_str())
            .collect();
        if self.upgrades.len() != 2
            || actions != BTreeSet::from(["WIX_DOWNGRADE_DETECTED", "WIX_UPGRADE_DETECTED"])
            || self
                .upgrades
                .iter()
                .any(|item| !same_guid(&item.code, UPGRADE_CODE))
        {
            return Err(UpdateError::Archive);
        }
        Ok(code)
    }

    fn component(&self, name: &str, key: &str, attributes: &str) -> Result<(), UpdateError> {
        let matches: Vec<_> = self
            .components
            .iter()
            .filter(|component| component.id == name)
            .collect();
        if matches.len() != 1
            || !guid(&matches[0].guid)
            || matches[0].directory != "INSTALLFOLDER"
            || matches[0].keypath != key
            || matches[0].attributes != attributes
        {
            return Err(UpdateError::Archive);
        }
        Ok(())
    }
}
