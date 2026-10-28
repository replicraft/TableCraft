// Finder receives documents through AppKit events, rather than CLI argv.
import AppKit
import Darwin
import Foundation

private let maximumRequestBytes = 1024 * 1024
private let maximumFiles = 128
private let requestPrefix = "tablecraft-open-"
private let launcherURL = URL(fileURLWithPath: CommandLine.arguments[0])
    .standardizedFileURL.resolvingSymlinksInPath()
private let cliURL = launcherURL.deletingLastPathComponent().deletingLastPathComponent()
    .appendingPathComponent("Resources/tablecraft")

private func cleanup(_ directory: URL) {
    try? FileManager.default.removeItem(at: directory.appendingPathComponent("request.plist"))
    try? FileManager.default.removeItem(at: directory.appendingPathComponent("Inspect.command"))
    // Never recursively remove a path received through an open request.
    directory.path.withCString { _ = rmdir($0) }
}

private struct OpenRequest: Codable {
    let executable: String
    let files: [String]
}

private func shellQuote(_ value: String) -> String {
    return "'" + value.replacingOccurrences(of: "'", with: "'\\''") + "'"
}

private func report(_ error: Error) {
    FileHandle.standardError.write(Data("TableCraft: \(error.localizedDescription)\n".utf8))
}

private enum LauncherError: LocalizedError {
    case invalidRequest
    case missingTerminal

    var errorDescription: String? {
        switch self {
        case .invalidRequest:
            return "The file-opening request is invalid."
        case .missingTerminal:
            return "Terminal is unavailable. Run tablecraft inspect from a terminal."
        }
    }
}

private func runRequest(_ path: String) throws -> Int32 {
    let requestURL = URL(fileURLWithPath: path)
    let directory = requestURL.deletingLastPathComponent()
    let temporary = FileManager.default.temporaryDirectory.resolvingSymlinksInPath()
    guard requestURL.lastPathComponent == "request.plist",
          directory.lastPathComponent.hasPrefix(requestPrefix),
          directory.deletingLastPathComponent().standardizedFileURL.resolvingSymlinksInPath().path == temporary.standardizedFileURL.path else {
        throw LauncherError.invalidRequest
    }

    let directoryAttributes = try FileManager.default.attributesOfItem(atPath: directory.path)
    let attributes = try FileManager.default.attributesOfItem(atPath: path)
    guard directoryAttributes[.type] as? FileAttributeType == .typeDirectory,
          (directoryAttributes[.ownerAccountID] as? NSNumber)?.uint32Value == getuid(),
          (directoryAttributes[.posixPermissions] as? NSNumber)?.intValue == 0o700,
          attributes[.type] as? FileAttributeType == .typeRegular,
          (attributes[.ownerAccountID] as? NSNumber)?.uint32Value == getuid(),
          (attributes[.posixPermissions] as? NSNumber)?.intValue == 0o600,
          let size = attributes[.size] as? NSNumber,
          size.intValue > 0, size.intValue <= maximumRequestBytes else {
        throw LauncherError.invalidRequest
    }
    defer { cleanup(directory) }
    let handle = try FileHandle(forReadingFrom: requestURL)
    defer { try? handle.close() }
    let data = try handle.read(upToCount: maximumRequestBytes + 1) ?? Data()
    guard !data.isEmpty, data.count <= maximumRequestBytes else {
        throw LauncherError.invalidRequest
    }
    let request = try PropertyListDecoder().decode(OpenRequest.self, from: data)
    guard request.files.count <= maximumFiles,
          request.executable.hasPrefix("/"),
          URL(fileURLWithPath: request.executable).standardizedFileURL.resolvingSymlinksInPath().path
            == cliURL.standardizedFileURL.resolvingSymlinksInPath().path,
          request.files.allSatisfy({ $0.hasPrefix("/") }) else {
        throw LauncherError.invalidRequest
    }

    var status: Int32 = 0
    let inputs: [String?] = request.files.isEmpty ? [nil] : request.files.map { $0 }
    for file in inputs {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: request.executable)
        process.arguments = file.map { ["open", "--wait", "--", $0] } ?? ["--help"]
        process.standardInput = FileHandle.standardInput
        process.standardOutput = FileHandle.standardOutput
        process.standardError = FileHandle.standardError
        try process.run()
        process.waitUntilExit()
        if process.terminationStatus != 0 {
            status = process.terminationStatus
        }
    }
    if request.files.isEmpty && isatty(STDIN_FILENO) != 0 {
        print("\nPress Return to close.", terminator: "")
        fflush(stdout)
        _ = readLine()
    }
    return status
}

private final class ApplicationDelegate: NSObject, NSApplicationDelegate {
    private var receivedDocuments = false
    private var pendingLaunches = 0

    func applicationDidFinishLaunching(_ notification: Notification) {
        // Pending Finder open events arrive during launch. A direct app launch shows help.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
            if !self.receivedDocuments {
                if !self.launch(files: []) {
                    self.terminateWhenIdle()
                }
            }
        }
    }

    func application(_ sender: NSApplication, openFiles filenames: [String]) {
        receivedDocuments = true
        guard filenames.count <= maximumFiles else {
            sender.reply(toOpenOrPrint: .failure)
            return
        }
        let accepted = launch(files: filenames.map { URL(fileURLWithPath: $0).path })
        sender.reply(toOpenOrPrint: accepted ? .success : .failure)
        if !accepted {
            terminateWhenIdle()
        }
    }

    private func terminateWhenIdle() {
        DispatchQueue.main.async {
            if self.pendingLaunches == 0 {
                NSApplication.shared.terminate(nil)
            }
        }
    }

    private func launch(files: [String]) -> Bool {
        var createdDirectory: URL?
        do {
            guard let terminal = NSWorkspace.shared.urlForApplication(withBundleIdentifier: "com.apple.Terminal") else {
                throw LauncherError.missingTerminal
            }
            let directory = FileManager.default.temporaryDirectory
                .appendingPathComponent(requestPrefix + UUID().uuidString, isDirectory: true)
            try FileManager.default.createDirectory(
                at: directory, withIntermediateDirectories: false,
                attributes: [.posixPermissions: 0o700]
            )
            createdDirectory = directory
            let request = OpenRequest(
                executable: cliURL.path,
                files: files
            )
            let requestURL = directory.appendingPathComponent("request.plist")
            let data = try PropertyListEncoder().encode(request)
            guard data.count <= maximumRequestBytes else {
                throw LauncherError.invalidRequest
            }
            try data.write(to: requestURL, options: .atomic)
            try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: requestURL.path)

            // Database filenames exist only in the plist and Process.arguments, never shell text.
            let command = directory.appendingPathComponent("Inspect.command")
            let script = "#!/bin/sh\nexec " + shellQuote(launcherURL.path)
                + " --terminal-request " + shellQuote(requestURL.path) + "\n"
            try Data(script.utf8).write(to: command)
            try FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: command.path)

            pendingLaunches += 1
            NSWorkspace.shared.open(
                [command], withApplicationAt: terminal,
                configuration: NSWorkspace.OpenConfiguration()
            ) { _, error in
                DispatchQueue.main.async {
                    self.pendingLaunches -= 1
                    if let error = error {
                        cleanup(directory)
                        self.show(error)
                    }
                    self.terminateWhenIdle()
                }
            }
            return true
        } catch {
            if let directory = createdDirectory {
                cleanup(directory)
            }
            show(error)
            return false
        }
    }

    private func show(_ error: Error) {
        let alert = NSAlert()
        alert.messageText = "TableCraft could not open the database"
        alert.informativeText = error.localizedDescription
        alert.runModal()
    }
}

if CommandLine.arguments.count == 3 && CommandLine.arguments[1] == "--terminal-request" {
    do {
        exit(try runRequest(CommandLine.arguments[2]))
    } catch {
        report(error)
        exit(1)
    }
}

private let application = NSApplication.shared
private let delegate = ApplicationDelegate()
application.delegate = delegate
application.setActivationPolicy(.accessory)
application.run()
