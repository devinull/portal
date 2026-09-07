# Portal roadmap

Portal's long-term direction is a terminal-native control plane for reaching
machines. This roadmap describes product intent rather than a release promise;
features are promoted into versioned plans only when implementation and
platform validation are scheduled.

## Private-network providers

Extract discovery, identity, addressing, and reachability behind a provider
interface. Tailscale remains the first and best-tested provider. ZeroTier is the
first planned expansion, followed by other private-network services when their
local APIs can provide stable device identity and safe, read-only discovery.

Provider support must preserve Portal's current privacy rules: no credentials,
raw status documents, private network names, or provider tokens are persisted.
The interface should allow one provider at a time initially and avoid merging
machines across networks until identity collisions are explicitly designed.

## SSH and remote terminals

Add SSH as a first-class action beside desktop and file transfer. Portal should
invoke the system OpenSSH client directly and honor the user's existing SSH
configuration, host-key policy, keys, and hardware-backed credentials. Portal
must not store SSH passwords or private keys.

The terminal design should support both people and AI agents:

- interactive PTY sessions and explicit noninteractive command execution;
- previewable executable and argument vectors before remote execution;
- opt-in working-directory and environment forwarding through allowlists;
- resumable sessions through established tools such as `tmux`;
- clear separation between local commands, remote commands, and transferred
  input;
- bounded output and cancellation behavior suitable for automation;
- no SSH agent forwarding by default, with visible risk guidance when enabled;
- auditable session metadata without recording terminal contents or secrets;
  and
- capability detection so an agent can determine whether interactive input,
  file transfer, or a persistent session is available.

## Additional access modes

Later releases may add provider diagnostics, port forwarding, remote command
profiles, and user-defined actions. Every action must remain previewable, use
separate process arguments instead of a shell, and keep credentials under the
operating system or provider's control.
