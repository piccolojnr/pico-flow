from flow.desktop import set_autostart


def test_autostart_entry_can_be_enabled_and_removed(tmp_path):
    launcher = tmp_path / "bin" / "flow linux"
    entry = set_autostart(True, config_home=tmp_path / "config", launcher=launcher)
    text = entry.read_text(encoding="utf-8")
    assert "Terminal=false" in text
    assert 'Exec="' + str(launcher) + '" --background' in text

    set_autostart(False, config_home=tmp_path / "config", launcher=launcher)
    assert not entry.exists()
