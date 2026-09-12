"""Push-to-talk chord state tracking, separated from the global-key backend."""


class ChordState:
    """Track push-to-talk and the overlapping Ctrl+Super+Space toggle chord."""

    def __init__(
        self, on_start, on_stop, shortcut="ctrl+super", on_handsfree_toggle=None,
        handsfree_enabled=True, is_handsfree_active=None,
    ):
        self.on_start = on_start
        self.on_stop = on_stop
        self.required = set(shortcut.split("+"))
        self.on_handsfree_toggle = on_handsfree_toggle or (lambda: None)
        self.handsfree_enabled = handsfree_enabled
        self.is_handsfree_active = is_handsfree_active or (lambda: False)
        self.pressed = set()
        self.active = False

    @staticmethod
    def _kind(key):
        value = str(key).lower()
        if "ctrl" in value:
            return "ctrl"
        if "cmd" in value or "super" in value or "win" in value:
            return "super"
        if "alt" in value or "option" in value:
            return "alt"
        if "shift" in value:
            return "shift"
        return None

    @staticmethod
    def _is_space(key):
        value = str(key).lower()
        return value in {" ", "space", "key.space"} or value.endswith(".space")

    def _pair_down(self):
        kinds = {self._kind(key) for key in self.pressed}
        return self.required <= kinds

    def _handsfree_down(self):
        kinds = {self._kind(key) for key in self.pressed}
        return {"ctrl", "super"} <= kinds and any(
            self._is_space(key) for key in self.pressed
        )

    def press(self, key):
        before_pair = self._pair_down()
        before_handsfree = self._handsfree_down()
        self.pressed.add(key)
        after_pair = self._pair_down()
        after_handsfree = self._handsfree_down()

        if self.handsfree_enabled and after_handsfree and not before_handsfree:
            # If modifiers arrived first, the existing PTT recording is promoted
            # to hands-free. Its later modifier-release callback is ignored by
            # the controller while it is in hands-free mode.
            self.on_handsfree_toggle()
        elif (
            after_pair and not before_pair
            and (not after_handsfree or not self.handsfree_enabled)
            and not self.active
        ):
            if not self.is_handsfree_active():
                self.active = True
                self.on_start()

    def release(self, key):
        before_pair = self._pair_down()
        self.pressed.discard(key)
        after_pair = self._pair_down()
        if before_pair and not after_pair and self.active:
            self.active = False
            self.on_stop()


class PynputHotkeyListener:
    def __init__(
        self, shortcut, on_start, on_stop, on_handsfree_toggle=None,
        handsfree_enabled=True, is_handsfree_active=None,
    ):
        self.shortcut = shortcut
        self.state = ChordState(
            on_start, on_stop, shortcut, on_handsfree_toggle,
            handsfree_enabled, is_handsfree_active,
        )

    def run(self):
        try:
            from pynput import keyboard
        except Exception as exc:
            print(f"[Flow] Global hotkey unavailable: {exc}")
            return

        suffix = " and Ctrl+Super+Space" if self.state.handsfree_enabled else ""
        print(f"[Flow] Hold {self.shortcut.upper()} to dictate{suffix}.")
        with keyboard.Listener(on_press=self.state.press, on_release=self.state.release) as listener:
            listener.join()
