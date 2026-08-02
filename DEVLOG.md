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
