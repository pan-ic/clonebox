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

## 2026/07/16 Quiet session
Had difficulties to write integration tests for 2 main reasons: clone() creates a new process so attempt any to compare directly what happens into
the child to the parent are inefficient and, current code organization doesn't allow to isolate some variable (i.e. PID) and needs some state
management to make some integration tests possible. Also have to keep in mind that there are 2 exits status, the parent and the child one.
