# Application templates, parts and visual wizards

Status: future requirements, reviewed 18 October 2026. No form/report/label designer,
template instantiator or native object writer is implemented.

Add guided creation inside design/forms/reports/lifecycle owners with reusable
components represented by supported application models. No crate per wizard.
Initially qualify a bound form, parent form with linked child subform, grouped
report and labels. Table Analyzer and broader application parts follow actual
schema/migration support.

Generate real supported objects, not only visual previews. Parent/child workflows
need master/child link fields, keys, control/data sources, navigation and update
semantics. Instantiation must handle name collisions, identities, dependencies,
references and startup metadata. Preview proposed objects/changes before apply;
schema restructuring uses [migration safeguards](DATABASE_ANALYSIS.md).

Reusable components need provenance for assets and active content. Generated
handlers/macros require language/host support and execution authorization. Unknown
objects require preservation or explicit rejection. Runtime-only application
publishing is a separate distribution workflow; hiding a designer is not security.

Acceptance includes model/object tests, binding/events, collisions/failures and
native save/reopen per Access profile. Report/label cases need printer, paper,
font and pagination evidence. A preview/headless build does not prove native
authoring. No wizard/application part is qualified today.

Microsoft documents [Form Wizard](https://support.microsoft.com/en-us/access/create-a-form-by-using-the-form-wizard),
[main forms/subforms](https://support.microsoft.com/en-us/access/create-a-form-that-contains-a-subform-a-one-to-many-form)
and [reusable design elements](https://support.microsoft.com/en-us/access/save-and-reuse-database-design-elements).
See [distribution profiles](ACCESS_DISTRIBUTION.md) and
[application evidence](../testing/ACCESS_APPLICATIONS.md).
