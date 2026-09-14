// ============================================================
//  SmartNet // git-remote-dev - Git Remote Helper для протокола dev://.
//
//  Git из коробки умеет незнакомые схемы: увидев `dev://`, он ищет в PATH
//  бинарник `git-remote-dev` и передаёт ему управление (протокол
//  gitremote-helpers(7) через stdin/stdout). Утилита ультра-лёгкая: живёт
//  ровно на время clone/push/pull и общается с запущенным клиентом SmartNet
//  через loopback-IPC (порт и токен - в ~/.config/smartnet/devhub-ipc.json).
//
//  Воркфлоу разработчика НЕ меняется:
//      git clone dev://technolog/smart-swarm
//      git add . && git commit -m "…"
//      git push origin main
//
//  Под капотом:
//   - clone/pull: helper берёт у клиента полный git-bundle репозитория и
//     распаковывает объекты в локальный .git (`git bundle unbundle`);
//   - push: helper пакует ветки в bundle (`git bundle create`), шлёт клиенту,
//     тот подписывает обновлённый индекс ключом кошелька (seq+1, анти-откат)
//     и анонсирует в Mainline DHT.
//
//  Author: TechnoL0g
//  Created: 2026-06
// ============================================================

use serde::Deserialize;
use std::io::{BufRead, Read, Write};
use std::process::Command;

#[derive(Deserialize)]
struct Discovery {
    port: u16,
    token: String,
}

#[derive(Deserialize)]
struct RefsResponse {
    exists: bool,
    refs: Vec<RemoteRef>,
}

#[derive(Deserialize)]
struct RemoteRef {
    sha: String,
    name: String,
}

fn die(msg: &str) -> ! {
    eprintln!("git-remote-dev: {}", msg);
    std::process::exit(1);
}

/// Порт и токен работающего клиента SmartNet.
fn discovery() -> Discovery {
    let base = if let Ok(appdata) = std::env::var("APPDATA") {
        std::path::PathBuf::from(appdata)
    } else if let Ok(home) = std::env::var("HOME") {
        std::path::PathBuf::from(home).join(".config")
    } else {
        die("не найден HOME/APPDATA");
    };
    let path = base.join("smartnet").join("devhub-ipc.json");
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        die("клиент SmartNet не запущен (нет discovery-файла devhub-ipc.json) - откройте приложение и повторите")
    });
    serde_json::from_str(&raw).unwrap_or_else(|_| die("повреждён devhub-ipc.json"))
}

/// dev://user/repo → (user, repo). Git может передать и "dev::<url>"-остаток.
fn parse_url(url: &str) -> (String, String) {
    let rest = url
        .strip_prefix("dev://")
        .or_else(|| url.strip_prefix("dev:"))
        .unwrap_or(url)
        .trim_matches('/');
    let mut it = rest.splitn(2, '/');
    let user = it.next().unwrap_or("").to_string();
    let repo = it.next().unwrap_or("").trim_end_matches(".git").to_string();
    if user.is_empty() || repo.is_empty() {
        die("ожидается dev://<username>/<repo>");
    }
    (user, repo)
}

fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Запустить git и вернуть stdout (stderr транслируем пользователю).
fn git(args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .output()
        .map_err(|e| format!("git не найден в PATH: {}", e))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn ipc_refs(d: &Discovery, user: &str, repo: &str) -> RefsResponse {
    let url = format!(
        "http://127.0.0.1:{}/refs?user={}&repo={}",
        d.port,
        url_encode(user),
        url_encode(repo)
    );
    let resp = ureq::get(&url)
        .set("X-Auth", &d.token)
        .call()
        .unwrap_or_else(|e| die(&format!("IPC-запрос refs: {}", e)));
    let body = resp.into_string().unwrap_or_default();
    serde_json::from_str(&body).unwrap_or_else(|_| die("некорректный ответ /refs"))
}

fn ipc_bundle(d: &Discovery, user: &str, repo: &str) -> Vec<u8> {
    let url = format!(
        "http://127.0.0.1:{}/bundle?user={}&repo={}",
        d.port,
        url_encode(user),
        url_encode(repo)
    );
    match ureq::get(&url).set("X-Auth", &d.token).call() {
        Ok(resp) => {
            let mut bytes = Vec::new();
            resp.into_reader()
                .read_to_end(&mut bytes)
                .unwrap_or_else(|e| die(&format!("чтение bundle: {}", e)));
            bytes
        }
        Err(ureq::Error::Status(_, resp)) => {
            die(&resp.into_string().unwrap_or_else(|_| "bundle не найден".into()))
        }
        Err(e) => die(&format!("IPC-запрос bundle: {}", e)),
    }
}

fn ipc_push(d: &Discovery, user: &str, repo: &str, bundle: &[u8], refs_json: &str, commits: u32) -> Result<(), String> {
    let url = format!(
        "http://127.0.0.1:{}/push?user={}&repo={}&commits={}&refs={}",
        d.port,
        url_encode(user),
        url_encode(repo),
        commits,
        url_encode(refs_json)
    );
    match ureq::post(&url)
        .set("X-Auth", &d.token)
        .set("Content-Type", "application/octet-stream")
        .send_bytes(bundle)
    {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(_, resp)) => Err(resp.into_string().unwrap_or_else(|_| "push отклонён".into())),
        Err(e) => Err(format!("IPC-запрос push: {}", e)),
    }
}

/// Распаковать bundle в локальный ODB (`git bundle unbundle` пишет объекты
/// в .git текущего репозитория; GIT_DIR git выставляет helper'у сам).
fn fetch_bundle(d: &Discovery, user: &str, repo: &str) {
    let bytes = ipc_bundle(d, user, repo);
    let tmp = std::env::temp_dir().join(format!("dev-{}-{}-{}.bundle", user, repo, std::process::id()));
    std::fs::write(&tmp, &bytes).unwrap_or_else(|e| die(&format!("tmp bundle: {}", e)));
    let res = git(&["bundle", "unbundle", tmp.to_str().unwrap()]);
    let _ = std::fs::remove_file(&tmp);
    if let Err(e) = res {
        die(&format!("git bundle unbundle: {}", e));
    }
}

/// Обработать push-батч: собрать bundle из веток и отдать клиенту.
fn do_push(d: &Discovery, user: &str, repo: &str, specs: &[String], out: &mut impl Write) {
    let mut srcs: Vec<(String, String)> = Vec::new(); // (src, dst)
    for spec in specs {
        let spec = spec.trim_start_matches('+');
        let (src, dst) = match spec.split_once(':') {
            Some((s, d2)) => (s.to_string(), d2.to_string()),
            None => (spec.to_string(), spec.to_string()),
        };
        if src.is_empty() {
            let _ = writeln!(out, "error {} удаление веток пока не поддерживается", dst);
            continue;
        }
        srcs.push((src, dst));
    }
    if srcs.is_empty() {
        let _ = writeln!(out);
        let _ = out.flush();
        return;
    }
    // Полный самодостаточный bundle всех пушимых рефов.
    let tmp = std::env::temp_dir().join(format!("dev-push-{}-{}.bundle", repo, std::process::id()));
    let mut args: Vec<&str> = vec!["bundle", "create", tmp.to_str().unwrap()];
    let src_names: Vec<String> = srcs.iter().map(|(s, _)| s.clone()).collect();
    for s in &src_names {
        args.push(s);
    }
    if let Err(e) = git(&args) {
        for (_, dst) in &srcs {
            let _ = writeln!(out, "error {} {}", dst, e.replace('\n', " "));
        }
        let _ = writeln!(out);
        let _ = out.flush();
        return;
    }
    // Рефы: sha каждого src под именем dst.
    let mut refs: Vec<serde_json::Value> = Vec::new();
    for (src, dst) in &srcs {
        let sha = git(&["rev-parse", src]).unwrap_or_default();
        refs.push(serde_json::json!({ "sha": sha, "name": dst }));
    }
    let commits: u32 = git(&["rev-list", "--count", &src_names[0]])
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let bundle = std::fs::read(&tmp).unwrap_or_else(|e| die(&format!("чтение bundle: {}", e)));
    let _ = std::fs::remove_file(&tmp);
    match ipc_push(d, user, repo, &bundle, &serde_json::to_string(&refs).unwrap(), commits) {
        Ok(()) => {
            for (_, dst) in &srcs {
                let _ = writeln!(out, "ok {}", dst);
            }
        }
        Err(e) => {
            for (_, dst) in &srcs {
                let _ = writeln!(out, "error {} {}", dst, e.replace('\n', " "));
            }
        }
    }
    let _ = writeln!(out);
    let _ = out.flush();
}

fn main() {
    // git вызывает: git-remote-dev <remote-name> <url>
    let args: Vec<String> = std::env::args().collect();
    let url = args.get(2).or_else(|| args.get(1)).cloned().unwrap_or_default();
    let (user, repo) = parse_url(&url);
    let d = discovery();

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut fetch_requested = false;
    let mut push_specs: Vec<String> = Vec::new();

    for line in stdin.lock().lines() {
        let line = line.unwrap_or_default();
        let cmd = line.trim();
        match cmd {
            "capabilities" => {
                let _ = writeln!(stdout, "fetch\npush\n");
                let _ = stdout.flush();
            }
            "list" | "list for-push" => {
                let r = ipc_refs(&d, &user, &repo);
                if !r.exists && cmd == "list" {
                    die(&format!("репозиторий dev://{}/{} не найден", user, repo));
                }
                let mut has_main = false;
                for rf in &r.refs {
                    if rf.name == "refs/heads/main" {
                        has_main = true;
                    }
                    let _ = writeln!(stdout, "{} {}", rf.sha, rf.name);
                }
                if has_main {
                    let _ = writeln!(stdout, "@refs/heads/main HEAD");
                } else if let Some(first) = r.refs.first() {
                    let _ = writeln!(stdout, "@{} HEAD", first.name);
                }
                let _ = writeln!(stdout);
                let _ = stdout.flush();
            }
            "" => {
                // Пустая строка завершает fetch/push-батч.
                if fetch_requested {
                    fetch_bundle(&d, &user, &repo);
                    fetch_requested = false;
                    let _ = writeln!(stdout);
                    let _ = stdout.flush();
                } else if !push_specs.is_empty() {
                    let specs = std::mem::take(&mut push_specs);
                    do_push(&d, &user, &repo, &specs, &mut stdout);
                } else {
                    break;
                }
            }
            _ if cmd.starts_with("fetch ") => {
                // Батчим: объекты приедут одним bundle'ом на пустой строке.
                fetch_requested = true;
            }
            _ if cmd.starts_with("push ") => {
                push_specs.push(cmd[5..].to_string());
            }
            _ if cmd.starts_with("option ") => {
                let _ = writeln!(stdout, "unsupported");
                let _ = stdout.flush();
            }
            _ => {
                let _ = writeln!(stdout);
                let _ = stdout.flush();
            }
        }
    }
}
