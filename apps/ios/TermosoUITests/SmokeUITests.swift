import XCTest

/// Golden path on a fresh profile: welcome → offline → add a host → find it →
/// settings. Runs against the real Rust core in the simulator.
final class SmokeUITests: XCTestCase {
    private var app: XCUIApplication!

    override func setUpWithError() throws {
        continueAfterFailure = false
        app = XCUIApplication()
        app.launchArguments = ["--ui-test"]
        app.launch()
    }

    func testWelcomeOfflineAddHostAndSettings() throws {
        snap("01-welcome")
        let offline = el("welcome.offline")
        XCTAssertTrue(offline.waitForExistence(timeout: 20), "welcome did not appear (vault open failed?)")
        offline.tap()

        let hosts = el("vaults.hosts")
        XCTAssertTrue(hosts.waitForExistence(timeout: 10))
        snap("02-vaults")
        hosts.tap()

        XCTAssertTrue(el("hosts.empty").waitForExistence(timeout: 5)
            || app.staticTexts["No hosts yet"].waitForExistence(timeout: 5))
        snap("03-hosts-empty")

        el("hosts.add").tap()
        let newHost = el("hosts.newHost")
        XCTAssertTrue(newHost.waitForExistence(timeout: 5))
        newHost.tap()

        let label = el("hostEditor.label")
        XCTAssertTrue(label.waitForExistence(timeout: 5))
        label.tap()
        label.typeText("pqssh")
        let address = el("hostEditor.address")
        address.tap()
        address.typeText("pq.example.org")
        let username = el("hostEditor.username")
        if username.exists {
            username.tap()
            username.typeText("ubuntu")
        }
        snap("04-host-editor")
        el("hostEditor.save").tap()

        let row = el("hosts.host.pqssh")
        XCTAssertTrue(row.waitForExistence(timeout: 10), "saved host not listed")
        XCTAssertTrue(app.staticTexts["ubuntu@pq.example.org"].firstMatch.exists)
        snap("05-hosts-one")

        // Reopen to confirm the draft round-trips through the core.
        row.tap()
        let editLabel = el("hostEditor.label")
        XCTAssertTrue(editLabel.waitForExistence(timeout: 5))
        XCTAssertEqual(editLabel.value as? String, "pqssh")
        el("hostEditor.cancel").tap()

        app.tabBars.buttons["Settings"].tap()
        XCTAssertTrue(el("settings.theme").waitForExistence(timeout: 5))
        snap("06-settings")
        // About is the last section; List rows below the fold do not exist yet.
        let core = el("settings.coreVersion")
        for _ in 0 ..< 6 where !core.exists {
            app.swipeUp()
        }
        XCTAssertTrue(core.waitForExistence(timeout: 5), "About → Core row not reachable")
        snap("06-settings-about")
    }

    func testSelfHostedRequiresValidURL() throws {
        let selfHosted = el("welcome.selfHosted")
        XCTAssertTrue(selfHosted.waitForExistence(timeout: 20))
        selfHosted.tap()

        let url = el("welcome.selfHosted.url")
        XCTAssertTrue(url.waitForExistence(timeout: 5))
        // Toolbar items wrap the button in a container that carries the same
        // identifier but not the disabled state, so target the button itself.
        let cont = app.buttons.matching(identifier: "welcome.selfHosted.continue").firstMatch
        XCTAssertTrue(cont.waitForExistence(timeout: 5))
        XCTAssertFalse(cont.isEnabled, "Continue must stay disabled for an empty address")
        url.tap()
        url.typeText("termoso.example.com")
        XCTAssertTrue(cont.isEnabled, "bare host name should normalise to https://")
        snap("07-self-hosted")
        cont.tap()

        XCTAssertTrue(el("vaults.hosts").waitForExistence(timeout: 10))
    }

    /// Identifier lookup independent of the element type SwiftUI picks
    /// (List rows and toolbar menus vary between buttons, cells and others).
    private func el(_ identifier: String) -> XCUIElement {
        app.descendants(matching: .any).matching(identifier: identifier).firstMatch
    }

    private func snap(_ name: String) {
        let attachment = XCTAttachment(screenshot: app.screenshot())
        attachment.name = name
        attachment.lifetime = .keepAlways
        add(attachment)
    }
}
