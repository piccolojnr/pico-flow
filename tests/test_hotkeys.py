from flow.hotkeys import ChordState


def test_ctrl_super_starts_once_and_stops_when_either_key_is_released():
    events = []
    chord = ChordState(lambda: events.append("start"), lambda: events.append("stop"))

    chord.press("Key.ctrl_l")
    chord.press("Key.ctrl_l")
    chord.press("Key.cmd")
    chord.press("Key.cmd")
    assert events == ["start"]
    chord.release("Key.ctrl_l")
    assert events == ["start", "stop"]
    chord.release("Key.cmd")
    assert events == ["start", "stop"]


def test_either_modifier_order_and_configured_chord():
    events = []
    chord = ChordState(lambda: events.append("start"), lambda: events.append("stop"), "ctrl+alt")
    chord.press("Key.alt_l")
    chord.press("Key.ctrl_r")
    chord.release("Key.alt_l")
    assert events == ["start", "stop"]


def test_releasing_one_duplicate_modifier_does_not_stop_while_other_side_is_down():
    events = []
    chord = ChordState(lambda: events.append("start"), lambda: events.append("stop"))
    chord.press("Key.ctrl_l")
    chord.press("Key.ctrl_r")
    chord.press("Key.cmd")
    chord.release("Key.ctrl_l")
    assert events == ["start"]
    chord.release("Key.ctrl_r")
    assert events == ["start", "stop"]


def test_ctrl_super_space_when_modifiers_arrive_first_promotes_current_ptt():
    events = []
    chord = ChordState(
        lambda: events.append("ptt-start"), lambda: events.append("ptt-stop"),
        on_handsfree_toggle=lambda: events.append("handsfree-toggle"),
    )
    chord.press("Key.ctrl_l")
    chord.press("Key.cmd")
    chord.press("Key.space")
    assert events == ["ptt-start", "handsfree-toggle"]
    chord.release("Key.space")
    chord.release("Key.ctrl_l")
    chord.release("Key.cmd")
    # The controller ignores the PTT stop callback after promotion.
    assert events == ["ptt-start", "handsfree-toggle", "ptt-stop"]


def test_ctrl_super_space_when_space_arrives_first_uses_toggle_only():
    events = []
    chord = ChordState(
        lambda: events.append("ptt-start"), lambda: events.append("ptt-stop"),
        on_handsfree_toggle=lambda: events.append("handsfree-toggle"),
    )
    chord.press("Key.space")
    chord.press("Key.cmd")
    chord.press("Key.ctrl_l")
    chord.press("Key.ctrl_l")  # autorepeat cannot toggle twice
    assert events == ["handsfree-toggle"]


def test_handsfree_mode_suppresses_bare_ptt_but_toggles_on_triple_chord():
    events = []
    active = [True]
    chord = ChordState(
        lambda: events.append("ptt-start"), lambda: events.append("ptt-stop"),
        on_handsfree_toggle=lambda: events.append("handsfree-toggle"),
        is_handsfree_active=lambda: active[0],
    )
    chord.press("Key.ctrl_l")
    chord.press("Key.cmd")
    chord.release("Key.ctrl_l")
    chord.release("Key.cmd")
    assert events == []

    chord.press("Key.ctrl_l")
    chord.press("Key.cmd")
    chord.press("Key.space")
    assert events == ["handsfree-toggle"]


def test_disabling_handsfree_preserves_ctrl_super_push_to_talk():
    events = []
    chord = ChordState(
        lambda: events.append("start"), lambda: events.append("stop"),
        on_handsfree_toggle=lambda: events.append("handsfree"),
        handsfree_enabled=False,
    )
    chord.press("Key.space")
    chord.press("Key.ctrl_l")
    chord.press("Key.cmd")
    assert events == ["start"]
    chord.release("Key.ctrl_l")
    assert events == ["start", "stop"]


def test_handsfree_shortcut_stays_ctrl_super_space_with_custom_ptt_pair():
    events = []
    chord = ChordState(
        lambda: events.append("ptt-start"), lambda: events.append("ptt-stop"),
        shortcut="ctrl+alt", on_handsfree_toggle=lambda: events.append("handsfree"),
    )
    chord.press("Key.ctrl_l")
    chord.press("Key.cmd")
    chord.press("Key.space")
    assert events == ["handsfree"]
