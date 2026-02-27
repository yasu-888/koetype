import Foundation

private func assert(_ condition: @autoclosure () -> Bool, _ message: String) {
    if !condition() {
        fputs("Assertion failed: \(message)\n", stderr)
        exit(1)
    }
}

private func makeEvent(
    type: String,
    text: String? = nil,
    message: String? = nil,
    level: Double? = nil,
    status: String? = nil
) -> OverlayEvent {
    OverlayEvent(type: type, text: text, message: message, level: level, status: status, x: nil)
}

private func testRecordingStarted() {
    let start = OverlayModel(status: "idle", message: "待機中", detail: "old", audioLevel: 0.3)
    let event = makeEvent(type: "recording-started")
    let transition = OverlayEventReducer.reduce(model: start, event: event)

    assert(transition.model.status == "recording", "recording-started should set recording status")
    assert(transition.model.message == "", "recording-started should clear message")
    assert(transition.model.detail == "", "recording-started should clear detail")
    assert(transition.effects.shouldResize, "recording-started should resize")
    assert(transition.effects.shouldShow, "recording-started should show")
    assert(transition.shouldCancelIdleReset, "recording-started should cancel idle reset")
}

private func testRecordingStarting() {
    let start = OverlayModel(status: "idle", message: "待機中", detail: "old", audioLevel: 0.3)
    let event = makeEvent(type: "recording-starting")
    let transition = OverlayEventReducer.reduce(model: start, event: event)

    assert(transition.model.status == "recording", "recording-starting should set recording status")
    assert(transition.effects.shouldResize, "recording-starting should resize")
    assert(transition.effects.shouldShow, "recording-starting should show")
}

private func testRecordingStopped() {
    let start = OverlayModel(status: "recording", message: "x", detail: "keep", audioLevel: 0.5)
    let event = makeEvent(type: "recording-stopped")
    let transition = OverlayEventReducer.reduce(model: start, event: event)

    assert(transition.model.status == "transcribing", "recording-stopped should set transcribing")
    assert(transition.model.message == "", "recording-stopped should clear message")
    assert(transition.model.detail == "keep", "recording-stopped should keep detail")
    assert(transition.effects.shouldResize, "recording-stopped should resize")
    assert(transition.effects.shouldShow, "recording-stopped should show")
    assert(transition.shouldCancelIdleReset, "recording-stopped should cancel idle reset")
}

private func testCompletionEventsHideAndReset() {
    let start = OverlayModel(status: "transcribing", message: "m", detail: "d", audioLevel: 0.7)
    let done = OverlayEventReducer.reduce(model: start, event: makeEvent(type: "transcription-completed"))
    let pasted = OverlayEventReducer.reduce(model: start, event: makeEvent(type: "paste-completed"))

    for transition in [done, pasted] {
        assert(transition.model == .idle, "completion should reset to idle model")
        assert(transition.effects.shouldHide, "completion should hide")
        assert(transition.shouldCancelIdleReset, "completion should cancel idle reset")
    }
}

private func testErrorEvents() {
    let start = OverlayModel.idle
    let withMessage = OverlayEventReducer.reduce(
        model: start,
        event: makeEvent(type: "transcription-failed", message: "boom")
    )
    let withoutMessage = OverlayEventReducer.reduce(
        model: start,
        event: makeEvent(type: "recording-error")
    )

    assert(withMessage.model.status == "error", "error should set error status")
    assert(withMessage.model.message == "boom", "error should use provided message")
    assert(withMessage.effects.shouldResize && withMessage.effects.shouldShow, "error should resize and show")
    assert(withoutMessage.model.message == "エラー", "error should fallback to default message")
}

private func testPartialTranscription() {
    let start = OverlayModel.idle
    let transition = OverlayEventReducer.reduce(
        model: start,
        event: makeEvent(type: "partial-transcription", text: "hello")
    )

    assert(transition.model.detail == "hello", "partial-transcription should update detail")
    assert(transition.effects.shouldResize, "partial-transcription should resize")
    assert(transition.effects.shouldShow, "partial-transcription should show")
}

private func testAudioLevel() {
    let start = OverlayModel.idle
    let transition = OverlayEventReducer.reduce(
        model: start,
        event: makeEvent(type: "audio-level", level: 0.42)
    )

    assert(transition.model.audioLevel == 0.42, "audio-level should update audio level")
    assert(transition.effects.shouldShow, "audio-level should show")
    assert(!transition.effects.shouldResize, "audio-level should not resize")
}

private func testStatusEvent() {
    let start = OverlayModel.idle
    let idleTransition = OverlayEventReducer.reduce(
        model: start,
        event: makeEvent(type: "status", text: "t", message: "m", status: "idle")
    )
    let activeTransition = OverlayEventReducer.reduce(
        model: start,
        event: makeEvent(type: "status", status: "recording")
    )
    let unknownActiveTransition = OverlayEventReducer.reduce(
        model: start,
        event: makeEvent(type: "status", status: "recording-starting")
    )

    assert(idleTransition.model.message == "m", "status should update message")
    assert(idleTransition.model.detail == "t", "status should update detail")
    assert(idleTransition.effects.shouldResize, "status should resize")
    assert(idleTransition.effects.shouldHide, "idle status should hide")
    assert(idleTransition.shouldCancelIdleReset, "idle status should cancel idle reset")
    assert(activeTransition.effects.shouldShow, "active status should show")
    assert(unknownActiveTransition.effects.shouldShow, "unknown non-idle status should still show")
}

private func testPositionAndUnknownEvents() {
    let start = OverlayModel.idle
    let positionTransition = OverlayEventReducer.reduce(
        model: start,
        event: makeEvent(type: "position")
    )
    let unknownTransition = OverlayEventReducer.reduce(
        model: start,
        event: makeEvent(type: "unknown")
    )

    assert(positionTransition.effects.shouldReposition, "position should reposition")
    assert(positionTransition.model == start, "position should not mutate model")
    assert(unknownTransition.model == start, "unknown should not mutate model")
    assert(unknownTransition.effects == OverlayEffects(), "unknown should have no side effects")
}

private func runAllTests() {
    testRecordingStarting()
    testRecordingStarted()
    testRecordingStopped()
    testCompletionEventsHideAndReset()
    testErrorEvents()
    testPartialTranscription()
    testAudioLevel()
    testStatusEvent()
    testPositionAndUnknownEvents()
}

@main
struct OverlayReducerTestsMain {
    static func main() {
        runAllTests()
        print("floating-overlay reducer tests passed")
    }
}
