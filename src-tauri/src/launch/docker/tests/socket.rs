//! The host Docker socket passed through (#46): the mount and the init block.
use super::*;

const MOUNT: &str = "/var/run/docker.sock:/var/run/docker.sock";

fn socket_request(agent: AgentId) -> crate::models::LaunchRequest {
    let mut req = request(agent, EnvId::Docker, r"C:\Code\demo");
    req.docker_socket = true;
    req
}

#[test]
fn the_socket_is_mounted_next_to_the_folder_for_every_agent() {
    for agent in AgentId::ALL {
        let plan = plan(&socket_request(agent), &console(), &resolved(None)).unwrap();
        let args = docker_args(&script(&plan));
        let mount = args.iter().position(|arg| arg == MOUNT).unwrap();
        assert_eq!(args[mount - 1], "-v", "{agent:?}");
        assert!(
            args[mount - 2].starts_with(r"C:\Code\demo:/workspace"),
            "{agent:?}: {}",
            args[mount - 2]
        );
        assert_eq!(args.iter().filter(|arg| *arg == MOUNT).count(), 1);
        for arg in plan.args.iter().chain(&args) {
            assert!(!arg.is_empty() && !arg.contains('"'), "{arg}");
        }
    }
}

#[test]
fn the_init_block_runs_first_and_the_agent_last() {
    for agent in AgentId::ALL {
        let plan = plan(&socket_request(agent), &console(), &resolved(None)).unwrap();
        let init = env_value(&script(&plan), "TRAYGENT_INIT").unwrap();
        assert_eq!(init, init_script(agent, true), "{agent:?}");
        assert!(!init.contains('\r'));
        assert!(init.starts_with("mkdir -p /home/node/.local/bin\n"));
        let plain = init_script(agent, false);
        assert!(
            init.ends_with(&plain),
            "{agent:?}: the agent's own script follows"
        );
        let block = &init[..init.len() - plain.len()];
        // The CLI install only warns: a session without docker still starts.
        assert!(!block.contains("exit"), "{block}");
        assert!(block.ends_with("export PATH=\"/home/node/.local/bin:$PATH\"\n"));
    }
}

#[test]
fn the_init_block_follows_the_starter() {
    let init = init_script(AgentId::Opencode, true);
    for part in [
        "if [ ! -x /home/node/.local/bin/docker ]; then",
        "echo '[traygent] Installiere Docker CLI ...'",
        "curl -4 -fsSL https://download.docker.com/linux/static/stable/x86_64/ 2>/dev/null",
        "grep -oE 'docker-[0-9]+\\.[0-9]+\\.[0-9]+\\.tgz'",
        "[ -n \"$DVER\" ] || DVER=27.5.1",
        "| tar xz --strip-components=1 -C /home/node/.local/bin docker/docker",
        "chown node:node /home/node/.local/bin/docker",
        "WARNUNG: Docker CLI konnte nicht installiert werden",
        "if [ -S /var/run/docker.sock ]; then",
        "DSG=$(ls -n /var/run/docker.sock | awk '{print $4}')",
        "case \"x$DSG\" in x|x0) ;;",
        "groupadd -g \"$DSG\" hostdocker",
        "usermod -aG \"$DGN\" node",
        "runuser -u node -- sh -c 'test -r /var/run/docker.sock && test -w /var/run/docker.sock' || chmod a+rw /var/run/docker.sock",
        "Kein Docker-Socket im Container",
    ] {
        assert!(init.contains(part), "{part}");
    }
    assert_eq!(DOCKER_CLI_FALLBACK, "27.5.1");
}

#[test]
fn without_the_socket_nothing_changes() {
    for agent in AgentId::ALL {
        let req = request(agent, EnvId::Docker, r"C:\Code\demo");
        let text = script(&plan(&req, &console(), &resolved(None)).unwrap());
        assert!(!docker_args(&text)
            .iter()
            .any(|arg| arg.contains("docker.sock")));
        let init = env_value(&text, "TRAYGENT_INIT").unwrap();
        assert!(!init.contains("docker.sock") && !init.contains("Docker CLI"));
    }
}

#[test]
fn a_local_start_ignores_the_socket() {
    let mut req = request(AgentId::Claude, EnvId::Local, r"C:\Code\demo");
    req.docker_socket = true;
    let cli = Some((r"C:\Users\max\.local\bin\claude.exe", false));
    let text = script(&plan(&req, &console(), &resolved(cli)).unwrap());
    assert!(!text.contains("docker.sock"));
}
