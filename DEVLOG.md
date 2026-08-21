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

## 2026/07/21 Hard session
network_namespaces (7); veth (4); ip (8) add, link, route; netns; netlink (7); rtnetlink (7)
Network are isolated namespace on NIX. A physical network is associated to 1 namespace only, if transfered to another namespace then it stays
until last process dies then go back to original namesapce. So veth are virtual network devices that can be create into new namespace and the
communicate with physical network devices. Veth work with pairs. Man says: place one end of a pair in a namespace then the other in another
namespace then ip link add <p1-name> netns <p1-ns> type veth peer <p2-name> netns <p2-ns> to create or ip link set <p2-name> netns <p2-ns> if
already existing
(by moving a side); use ethool (8) to test.
Discovered network namespaces, veth pairs, netlink, and iptables NAT
from scratch. No prior assumptions to break pure discovery. Full
stack working: container namespace with veth pair, IP assignment,
loopback, default route, MASQUERADE NAT, internet access from inside
the namespace. Next: implement in Rust using rtnetlink then raw netlink.

## 2026/07/23 Hard session
To understand how veth network, ip and netns tools works I had to create manually first then, using std::process::Command I replicated by code
the manual steps. During the manual experiment a net namespace has been created manually. Thing that differs and bring some trouble with the
container is that the clone call with the CLONE_NEWNET flag creates an anonymous namespace that cannot be used with netns so I had to use nsenter.
The next steps are to change Command() use to rtnetlink; which is only a transition to understand the framework because that would need to switch
the actual code to async only for network creation so, the last step is to use directly unix socks to create the network.

## 2026/07/24 Quiet session
Apparently netlink_packet_route::link::LinkMessage exist behind packet::route::LinkMessage inside rtnetlink, had to check public re-exports.
Implementation using rtnetlink blocks on child setup because of AsyncSockets, in theory it could be implemented to finish the experiment but I use
raw socket as final implementation so I've mixed rtnetlink for parent + nsenter for child. Child would use exactly the same init step than parent but
after calling new_connection_with_socket() that is the remote connection to the child network.

## 2026/07/25-31 Hard session
Last part of the network core was to directly use raw socket and Kernel structs. I would qualify that first experience with Kernel network programming
as both passionate and frustrating. Frustration comes from:
- Often real difficulties to find sources about how things works. Documentation is often really poor if you don't know where to find it. So the rule I've
established for Kernel network programming (will be extended to Kernel programming) sourcing:
    a. man pages; if lucky, get the big picture of the data struct and implementation with the related pages
       (e.g. netlink(7)-(3), rtnetlink(7)-(3)...),
    b. github, OSS projects; if lucky, any public source code of a related  project used to check syscalls and implementation
       (e.g. runc, containerd, rtnetlink rust crate, netlink go package, iproute2..),
    c. strace/perf/bpftrace
       (e.g. strace -x -s 1000 -e sendmsg ip addr add 10.0.0.1/24 dev veth1 2>&1),
    d. minimal program; just write a minimal program that aims to use the tageted datastruct, that allows to focus/divide and give a better exploitation
       of the potential responses of the API/Kernel
    e. Kernel source code (bootlin), grep
       (e.g. bootlin '/net' 'drivers/net' '/include', 'include/uapi'; grep -r {} "/usr/include/linux"),
    f. Kernel documentation at /Documentation, examples at /samples,
    g. LWN.net for articles on Kernel features.
The faster method to me is to use strace then check docs/code.
- A lot of type casting in this kind of code, because of the use of Rust types and C types and the switch between stdlib, libc and netlink
type casting hell
- Sometimes C macros are not yet translated in Rust lib, grep -r "{}" /usr/include/linux is a life saver to get the scalar value there.
I've also been tricked by two things:
- socket messages queue, unread ACK made a mess with get_if_id() giving a stale id.
- when you use an C struct implemented in Rust and that implementation has private fields these have to be initialized (zeroed padding) using methods like
std::mem::zeroed()
At the end implementing from scratch had the great advantage to make the child network setup easier, it only needs to be accessed by opening an fd on
the child net ns then uses of setns to switch from parent to child, setting up and vice versa.

## 2026/08/02-04 Hard session
I've tried to create/write cgroups, everything worked almost fine until I had to move the child PID into the child cgroup.procs at 
/sys/fs/cgroups/clonebox/{container_name}/cgroup.procs . That is explained because the system.d handle the child resource after the clone() process 
(that can be checked using cat /proc/1/cgroup && systemd-cgls | head -30). To avoid that solution are either:
-tinkering moving clonebox to it's own scope: systemd-run --scope --slice=clonebox.slice ./target/debug/clonebox run --name test --cmd /bin/sh. This is not
acceptable for a serious container runtime demo that follows OCI; it would require that the user first find the problem in the doc and then tyoe that command
at every run.
-implementing clone3 properly that allows to:
"A child process created via fork(2) inherits its parent's cgroup memberships.  A process's cgroup memberships are preserved across execve(2).
The clone3(2) CLONE_INTO_CGROUP flag can be used to create a child process that begins its life in a different version 2 cgroup from the parent process"
(man 7 clone, NOTE)
The problem is: nix doesn't not implement clone3(). Glibc itself doesn't wrap the clone3(). I have to wrap it properly using syscalls + rust.

## 2026/08/05-06 Hard session
I've tried to implement a Clone3 version of the syscall with the same name. First idea was to develop it the clean way: generic and near from nix implementation
of clone, the goal was to ensure compatibility with almost the same function call (or minimal refactor). So the main idea was with the help of the builder pattern
to create a Rust wrapper (Clone3 struct) of the struct CloneArgs needed by the clone3 syscall. That Clone3 struct initially contains the callback that is sent to
the child. That implementation has a trade off directly linked to Rust memory managment which is:
-Clone3 struct in the parent is changed to CloneArgs, CloneArgs is a Rust "ffi" for the clone3 struct clone_args. That clone_struct has no clue about the Rust
callback (closure). This bring a first trouble: before the builder pattern drop Clone3 struct for CloneArgs the closure has to move inside .build() scope.
-The clone3 syscall is a fork style code, so the child inherits everything the parents allow the child to (because that's still clone, thanks to CloneFlags we have
control over what the child inherits). The stack will be inherited by the child and shared state exists between parent and child until we execute the callback (the 
execve a the end of the callback replace the process image: stack, heap, etc...). At some point parent process will leave the .build() method, this will destroy
/erase the callback. The child on his side will execute a callback that has been erased. Follows a segfault.
The current status of the implementation has a call back that is not really needed because it mostly follow fork style. 
Implementing a nix style clone would need here to use CLONE_VM but it break the container isolation core concept. Plus clone3 fork style make that impossible
to use functon-pointer style entry withou assembly 
Else we would have:
-create a raw pointer on the callback and optional arguments at the top of the child stack
-call clone3 with that stack
-start the child at a trampoline function that allow the child to start at that callback and not follow the rest of the instruction the paren had. The callback then
erase the stack.

## 2026/08/07 Quiet session
Finished cgroupv2 after clone3 implementation. Use of CLONE_INTO_CGROUP uses a file descriptor of the new leaf cgroup to associate that to the child
(no cgroup.procs). EBUSY will happen if the resource has been created into the leaf before it's associated to the child.

## 2026/08/08 Quiet session
Started to map command line to full OCI lifecycle. Created all needed functions, netx step implementation. State.json file writing/reading implementation. More
generaly bundle creation (file located a run/clonebox that will contain what's needed for runtime)

## 2026/08/09 Hard session
Implementation of IPC between processes (pipe + Unix socket) to follow OCI guidelines: create create a child process then freeze the child before it execute the command
and reset the process (then "becomes a real container"). Start unfreeze that child process writing a byte into a pipe between the child and the start process.
Order of the freeze call is critical:
-creates pipe between parent and non existing child first (but before clone anyway).
-pipe being unidirectionnal on end is used by parent to write to the child, the other end is use dby the child to read parent.
-the parent process has itself to be stopped before waiting it's child return (else deadlock), that happens when accepting connexion to a unix socket, then the start
command connect to that socket to unfreeze
-the parent write to the child, as the child read is before execve, execution of the child is unfreeze and the user command is launched in the container.
Order of the pipe2 (freeze) arguments is also critical, for the record:
Had reversed assignment causing EBADF on parent write. create blocks on accept(), start connects to socket, parent writes to pipe, child unblocks from read()
and calls execve. A lot of wasted debugging time.
Config parsing added, commands are now mapped via the config file and not anymore per command line

## 2026/08/10 Hard session:
Mount handling implementation has been difficult for many reasons:
-project architecture, that has to be reviewed, everything in the current state of the project imperative styl and some struct could be implemented now
to cluster related data and helpers/methods
-first I was handling bind per default which is not the required behavior. Things has been defaulted to overlayfs then mount follow what the user asks for
in the ocnfig file
-there is much much more on linux mount to implement that's outside of that project scope so the only mounts who actually works are tmpfs and bind
anything else will be defaulted to overlayfs (as default rootfs: upper/work/merged dirs created in bundle, lowerdir=root.path). Auto mount also happens 
with /proc, /sys and /dev.
-things are splitted into different calls, bind and overlayfs mount had ot happen before the pivot root, in a pre-pivot func. Then all other mounts
(tmpfs, sys, dev, proc) happens in posrt pivot, in the new view of the child process

## 2026/08/13 Quiet session:
Exec via setns

## 2026/08/14 Quiet session
Added kill/delete/pause/resume/state commands, state machine persisted to state.json

## 2026/08/15 Quiet session
Logger
Network fix: Veth names derived from container_id (was hardcoded, broke multi-container and tests). Integration tests: lifecycle happy path, error cases,
cgroups, network ping, filesystem bind mount.

## 2026/08/16 Hard session;
Integration tests with their individual config files. That's a bit of noise but I keep that this way for now by lack of time. The fact is that one config file
for every test is not very convenient because you might want to keep container running at different time. Adding different timer and config files allows to run
tests with threading else some resources conflict might happens because of execution flow. That's why for example (for convenience and complexity reasons) lifecycle()
test is a solo full function testing the whole state flow.
Had also some troubles to test things independently and with multi threading without making a real mess on the host (that's equivalent to spawn a big amount of
containers in a short time and due to well known limitation list, that creates a lot of conflicts)

## 2026/08/17 Hard session
Decided to enlarge project scope by creating a Cloneboxd daemon. The reason are that I hac rpc on my learning list since a while and had no real project that
actually brings that into my scope. The fact that a runtime is essentilaly a back end tool that shouldn't be directly communicated with + the RPC power to
generate all the runtime needed for data exchange betwen serve and client (no need to create a new API over clonebox) motivated me for the stack choose.
The actual question was how to split teh current code between three different parts: clonebox (cli/client binary), cloneboxd (daemon/server binary) and
clonebox-core (lib with all the core features that makes clonebox container runtime). I had to choose between 2 options on the spectrum of "what are the
daemon tasks":
-shim: that's how clonebox has been developped and the option I keep; the actual container runtime is also the parent/supervisor of the container. It holds a freeze
until it receive the signal to let the child execute his process. The deamon keeps a track of the differents existing runtime and update the state.
-daemon handled/absorbed: the daemon handle the runtime, the freeze and everyting. The point is that if the daemon crashes well everything crashes.
Finally bringing a daemon into the scope helps me handle a Clonebox weakness: the state managment is currently handled by file writing; this is a decent enough
solution for development purpose but brings problems:
-updates may not be that accurate
-a race condition may occur (ex 2 tasks modify the file almost in the same time, undefined behavior).
Now with the demaon state managment is owned  by only one process; the daemon. It's both loaded in memory and written. Keeping a file helps for persistence and
crash recovery.
Finally bringing a daemon into scope helps address a Clonebox weakness: state management is currently handled by file writing; decent for development but brings problems:
-updates may not be accurate
-race condition possible (2 tasks modify the file near-simultaneously, undefined behavior).
Target: state fully owned by cloneboxd (in-memory, file as write-through backup), supervisors report events instead of writing state themselves (single writer), race gone.
Sequencing: RPC skeleton ships first with a halfway version (daemon reads/serializes access to state.json, supervisors still write it directly) to validate
the architecture end-to-end. Full single-writer version comes after, once skeleton works.

## 2026/08/18 Quiet session
Clonebox repository split into: clonebox, cloneboxd and clonebox-core. clonebox becomes the client + cli that interfere with the daemon cloneboxd. Cloneboxd
manage the sate and keep tracks of the containers, communicate directly with the parent processes that supervise containers. Clonebox-core are the core library
that make container runtime working.
Code refactor and optimization planned soon for the clonebox-core

## 2026/08/19 Quiet session
Deep dive into protocol buffers documentation, communication between different service amde easy and language agnostic. .proto file are the "data scheme" that
you implement to serialize deserialize and communicate between services. Else data is sent as bytes over the wire. Then the app service will depend on a runtime
implemented in the language used by the app (ex: for rust: tonic + prost (+ tokyo because of async dep)). That runtime will generate all the structure needed to
load/deserialize, serialize data that is sent between app and needed accessor (getters/setters). That generated code is the compiled by the app.
The pros of protobuf and rpc are compatibilty (backward and forward) and flexibility (code + api generated followig the rule you declare in the 
.proto file). Cons I find is that it an really be tricky when use on prod because:
-deleting/updating are a pain in a prod situation because of backward and forward compatibity.
-special rules for encoding where lower values fields use less place, then if increase per range with the values
so the way the data is organized really matters and architecture must take into account potential future features.
It also might creates big files with a lot of redundancy
