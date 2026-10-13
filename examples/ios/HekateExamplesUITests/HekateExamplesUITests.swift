// SPDX-FileCopyrightText: 2026 Andrei Kochergin <andrei@oumuamua.dev>
// SPDX-FileCopyrightText: 2026 Oumuamua Labs <info@oumuamua.dev>
// SPDX-License-Identifier: AGPL-3.0-only

import XCTest

final class HekateExamplesUITests: XCTestCase {
    func testRsaSignatureMadeOnDeviceProvesAndVerifies() {
        let app = XCUIApplication()
        app.launch()

        runAndExpectThreeVerifiedProofs(app, attachmentPrefix: "rsa")
    }

    func testMlDsaSignatureMadeOnDeviceProvesAndVerifies() throws {
        let app = XCUIApplication()
        app.launch()

        app.segmentedControls["statement"].buttons["ML-DSA-65"].tap()

        let unavailable = app.staticTexts["unavailable"]

        guard !unavailable.exists else {
            throw XCTSkip(unavailable.label)
        }

        runAndExpectThreeVerifiedProofs(app, attachmentPrefix: "ml-dsa")
    }

    func testKeccakPreimageProvesAndVerifies() {
        let app = XCUIApplication()
        app.launch()

        app.segmentedControls["statement"].buttons["Keccak-256"].tap()

        runAndExpectThreeVerifiedProofs(app, attachmentPrefix: "keccak")
    }

    func testSha256PreimageProvesAndVerifies() {
        let app = XCUIApplication()
        app.launch()

        app.segmentedControls["statement"].buttons["SHA-256"].tap()

        runAndExpectThreeVerifiedProofs(app, attachmentPrefix: "sha256")
    }

    func testCancelStopsRunningProof() {
        let app = XCUIApplication()
        app.launch()

        app.buttons["run"].tap()
        app.buttons["cancel"].tap()

        XCTAssertTrue(
            app.staticTexts["cancelled"].waitForExistence(timeout: 30),
            "the run did not report Cancelled"
        )

        keep(XCTAttachment(screenshot: app.screenshot()), as: "cancelled")
    }

    private func runAndExpectThreeVerifiedProofs(_ app: XCUIApplication, attachmentPrefix: String) {
        app.buttons["run"].tap()

        let verdict = app.staticTexts["verdict"]
        let failure = app.staticTexts["failure"]

        guard verdict.waitForExistence(timeout: 180) else {
            return XCTFail(failure.exists ? failure.label : "no verdict within 180 seconds")
        }

        XCTAssertEqual(verdict.label, "3 of 3 proofs verified")

        keep(XCTAttachment(screenshot: app.screenshot()), as: "\(attachmentPrefix)-measurements")

        app.swipeUp()

        keep(XCTAttachment(screenshot: app.screenshot()), as: "\(attachmentPrefix)-conditions")
    }

    private func keep(_ attachment: XCTAttachment, as name: String) {
        attachment.name = name
        attachment.lifetime = .keepAlways

        add(attachment)
    }
}