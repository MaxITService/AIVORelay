# AivoRelay on Ubuntu GNOME Wayland

This guide adapts Handy's Ubuntu 26.04 GNOME Wayland troubleshooting to
AivoRelay's settings. These steps have not been verified with AivoRelay on
Ubuntu. Run commands individually and check their output before continuing.

## 1. Check the session and input permissions

```bash
echo "$XDG_SESSION_TYPE"
id
ls -l /dev/uinput
grep -R 'uinput' /etc/udev/rules.d /usr/lib/udev/rules.d 2>/dev/null
```

For the configuration below, the session should be Wayland and `/dev/uinput`
should be accessible to the `input` group (normally `root:input`, mode `0660`).
If that group is missing from your account, add it:

```bash
sudo usermod -aG input "$USER"
```

Save your work, then log out and back in. With autologin or a lingering user
manager, the systemd user manager can survive logout and start new applications
with its old group list. Check when it started:

```bash
systemctl show --property=MainPID --value "user@$(id -u).service"
ps -o pid,lstart -p "$(systemctl show --property=MainPID --value "user@$(id -u).service")"
```

If it predates the group change and permissions still fail after logging in,
a reboot refreshes the session and user manager. Alternatively, after saving
all work, restart the user manager:

```bash
sudo systemctl restart "user@$(id -u).service"
```

Restarting it stops your user services and may end the graphical session.
Reopen AivoRelay afterwards. Check group membership in a fresh terminal with
`id`; adding the group alone does not update already running processes.

## 2. Install and test text input tools

```bash
sudo apt install ydotool wl-clipboard
systemctl --user start ydotool
systemctl --user status ydotool --no-pager
```

If your package does not provide that user service, follow the service setup
for your installed ydotool package. The daemon must be running and its socket
accessible before the typing test can work:

```bash
ydotool type "HELLO FROM YDOTOOL"
```

Focus a disposable text field before running the test. Also check clipboard
support with `wl-copy` and a normal paste in the target application.

## 3. Configure AivoRelay

Use the application's settings rather than editing Handy's configuration file:

- Select **Direct** as the paste method, then select **ydotool** in **Typing Tool**.
  Typing Tool controls direct typing; it does not change the selected paste method.
- Keep your existing recording shortcut; test it in a disposable text field.
  If Ctrl+Space switches the input method instead, choose an unused combination
  in AivoRelay. Ctrl+Alt+Space is already used by the fork's Send action, so
  do not assign it to dictation without checking the other bindings.

The fork's Shortcut Engine selector is currently Windows-only. Linux uses its
Tauri shortcut backend; upstream's advice to select `handy_keys` is not supported
by this fork on Linux. Editing `shortcut_engine` or upstream's
`keyboard_implementation` does not enable that backend here. Handy's
`com.pais.handy` settings path and `handy` process commands also do not apply.

If GNOME does not allow the built-in global shortcut, bind the fork's existing
recording CLI in your desktop shortcut settings, using the full executable path:

```bash
/path/to/aivorelay recording toggle
```

AivoRelay must already be running. The same CLI accepts `recording start`,
`recording stop`, and `recording cancel`. A desktop shortcut invokes discrete
commands; it does not reproduce the application's hold-to-record behavior.

If GNOME focuses the recording overlay instead of the target application,
disable the recording overlay and retry. Test normal dictation before adding
LLM post-processing or a custom output route.

## 4. Non-QWERTY layouts

ydotool sends physical key codes. A paste shortcut can therefore press the wrong
key on layouts such as bepo or Dvorak. AivoRelay supports an **External Script**
paste method; it passes the completed text as the script's first argument.
For example, a bepo paste script can copy the text and press the physical key
that produces `v` on that layout:

```sh
#!/bin/sh
printf '%s' "$1" | wl-copy || exit 1
sleep 0.1
ydotool key 29:1 22:1 22:0 29:0
```

Save it as `~/.local/bin/aivorelay-paste`, make it executable with
`chmod +x ~/.local/bin/aivorelay-paste`, then select **External Script** as the
paste method and choose the script's full path. Verify key codes against your
layout before using it. Older ydotool releases use a different key syntax;
check `ydotool key --help` if the example is rejected.

Source: [Handy Ubuntu troubleshooting](https://github.com/cjpais/Handy/blob/16073813cdb141d0a365a67b4e521f636efde0ca/docs/troubleshooting/ubuntu-26-04-gnome-wayland/README.md).
