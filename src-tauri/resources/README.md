The release workflow drops the Linux `lazyaicode-agent` here before bundling
the Windows installer (see `tauri.windows.conf.json`). The app copies it into a
WSL distro (`~/.lazyaicode/lazyaicode-agent`) on first use. Empty in dev
builds: set `LAZYAICODE_AGENT_BIN` or install the agent in the distro by hand.
