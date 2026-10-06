// swift-tools-version: 6.0
import PackageDescription
import Foundation

let packageRoot = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
let rustLibraryDirectory = packageRoot
    .appendingPathComponent("../target/debug")
    .standardizedFileURL
    .path

let package = Package(
    name: "MikomaiDesktopMac",
    platforms: [.macOS(.v13)],
    products: [
        .executable(name: "MikomaiDesktopMac", targets: ["MikomaiDesktopMac"])
    ],
    dependencies: [
        // Supply the Testing macros explicitly and stay on 6.2: Testing 6.3
        // requires _TestingInterop, absent from the supported macOS 26.5 CLT.
        .package(url: "https://github.com/swiftlang/swift-testing.git", .upToNextMinor(from: "6.2.0"))
    ],
    targets: [
        .target(name: "MikomaiDesktopCore", dependencies: ["MikomaiBindings"], path: "Sources/MikomaiDesktopCore"),
        .target(name: "MikomaiGeneratedFFI", path: "Sources/MikomaiGeneratedFFI", publicHeadersPath:"include", linkerSettings:[.unsafeFlags(["-L\(rustLibraryDirectory)"]),.unsafeFlags(["-Xlinker","-rpath","-Xlinker","@executable_path/../Frameworks"]),.linkedLibrary("mikomai_bindings")]),
        .target(name:"MikomaiBindings",dependencies:["MikomaiGeneratedFFI"],path:"Sources/MikomaiBindings"),
        .executableTarget(
            name: "MikomaiDesktopMac",
            dependencies: ["MikomaiDesktopCore", "MikomaiBindings"],
            resources: [.copy("Resources/AppIcon.icns")]
        ),
        .testTarget(
            name: "MikomaiDesktopCoreTests",
            dependencies: [
                "MikomaiDesktopCore",
                .product(name: "Testing", package: "swift-testing")
            ]
        )
    ]
)
