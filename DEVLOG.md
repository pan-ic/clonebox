## 2026/07/10 Quiet session
I built a vertical slice pipeline using fork() + execve().
The CString wrapper adds a layer to the mental model when doing syscalls.
Tests + fix known issues next session (unhandled execve error, hardcoded command, empty env entry)

## 2026/07/13 Quiet session
Switched CLI from builder to derive pattern I think is cleaner for subcommand-heavy CLIs.
Discovered execve returns Result<Infallible> which makes if let Err(e) irrefutable, the correct pattern is
let Err(e) = ... else { unreachable!() }

## 2026/07/15 Quiet session
Switched fork() to clone() because it allows to customize the namespaces of the child process, that is a part
of the steps needed to isolate the future container. Debugged waitpid not blocking; the root cause was missing SIGCHLD signal in clone() call.
Known problem: migrate .unwrap() to .with_context()? + anyhow::Result<smtg>. Next: verify PID namespace isolation via /proc inode comparison.