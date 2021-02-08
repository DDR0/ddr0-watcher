Install:
- Run `cargo install --file .`
- Put `*.service` and `*.timer` in `~/.config/systemd/user/`.
- Run `systemctl --user enable ddr0-watcher.timer`

Development:
- Run `watch.sh` for automatic recompilation and execution.
- Run `systemctl --user daemon-reload` if you've edited the service files.
- Run `systemctl --user status ddr0-watcher.timer` to see the timer status.