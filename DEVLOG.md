## 2026/07/10 Quiet session
I built a vertical slice pipeline using fork() + execve().
The CString wrapper adds a layer to the mental model when doing syscalls.
Tests + fix known issues tomorrow (unhandled execve error, hardcoded command, empty env entry)