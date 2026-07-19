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

## 2026/07/17 Quiet session                                                                                                                          
Set up container UTS namespace to the container name, sethostname() might fail if name is invalid or right are not sufficient. In that case          
process exits. Another important part is that sethostname() must happen into the child process else it obviously change the parent process: your os.

## 2026/07/18-19 Hard session
Mounted the child fs. Everything is hardcoded for now because things will change with config parsing. The session was pretty difficult because
my assumption was:
- get fs copy
- pivot_root on child
- mount proc
Reality is different:
- very first ensure that the child process is be isolated from parent as mentioned in pivot_root
- bind mount the fs on itself because pivot_root need a mounted dir
- create an put_old dir that is inside the future fs. That's seems old but the old file system will be switched here so the old root becomes
accessible at /put_old inside the new root
- pivot root immediately followed by chdir, unmount put_old if not needed anymore then delete.
- finally mount proc on /proc
Even if there is no way to exec in the container yet, it's still possible to launch a shell inside the current one (clonebox run --name test
--cmd /bin/sh) that really really helped me to test syscall manually line per line.
