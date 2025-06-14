use reqwest::Client;
use reqwest::header::ACCEPT;
use reqwest::header::USER_AGENT;
use serde::{Deserialize, Serialize};
use std::{ffi::OsStr, fs, path::PathBuf, process::Command};

fn is_git_repo(path: &String) -> bool {
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries {
            match entry {
                Ok(entry) => {
                    if entry
                        .path()
                        .file_name()
                        .is_some_and(|subdir| subdir.eq(OsStr::new(".git")))
                    {
                        return true;
                    }
                }
                Err(_e) => (),
            }
        }
    }
    false
}
fn git_pull(local_repo: LocalRepo) -> Result<std::process::Output, std::io::Error> {
    return Command::new("git")
        .arg("pull")
        .current_dir(local_repo.path)
        .output();
}

fn git_clone(
    remote_repo: RemoteRepo,
    repo_base_dir: String,
    github_team_prefix: String,
) -> Result<std::process::Output, std::io::Error> {
    return Command::new("git")
        .arg("clone")
        .arg(remote_repo.git_url.clone())
        .arg(remote_repo.name.replace(github_team_prefix.as_str(), ""))
        .current_dir(repo_base_dir)
        .output();
}

#[warn(dead_code)]
fn is_directory_present(repo_dir: &String) -> bool {
    return match fs::metadata(repo_dir).map(|metadata| metadata.is_dir()) {
        Ok(is_dir) => is_dir,
        _ => false,
    };
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct RemoteRepo {
    name: String,
    archived: bool,
    git_url: String,
}

async fn get_repos(
    client: &Client,
    token: &String,
    page: i32,
    github_team_repo_url: &String,
) -> Option<Vec<RemoteRepo>> {
    let repos = client
        .get(github_team_repo_url)
        .header(ACCEPT, "application/vnd.github.v3+json")
        .header(USER_AGENT, "reposync")
        .bearer_auth(token)
        .query(&[("per_page", "100"), ("page", page.to_string().as_str())])
        .send()
        .await
        .ok()?
        .json::<Vec<RemoteRepo>>()
        .await
        .ok()?;
    if !repos.is_empty() {
        return Some(repos);
    }
    return None;
}

async fn list_github_team_repos(
    token: &String,
    github_team_repo_url: &String,
    github_team_prefix: &String,
) -> Vec<RemoteRepo> {
    let client = Client::new();
    let mut repos: Vec<RemoteRepo> = Vec::new();
    let mut page = 1;
    while let Some(page_repos) = get_repos(&client, &token, page, github_team_repo_url).await {
        repos.extend(page_repos);
        page += 1;
    }
    return repos
        .into_iter()
        .filter(|repo| !repo.archived)
        .filter(|repo| repo.name.starts_with(github_team_prefix.as_str()))
        .collect::<Vec<RemoteRepo>>();
}

#[derive(Debug, Clone)]
struct LocalRepo {
    name: String,
    path: PathBuf,
}

fn list_local_repos(path: &String) -> Vec<LocalRepo> {
    let mut repos: Vec<LocalRepo> = Vec::new();
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries {
            if let Ok(subdir) = entry {
                if is_git_repo(&subdir.path().to_string_lossy().to_string()) {
                    repos.push(LocalRepo {
                        name: subdir.file_name().into_string().unwrap(),
                        path: subdir.path(),
                    });
                }
            }
        }
    }
    repos
}

fn is_known_repo(
    remote_repo: &RemoteRepo,
    local_repos: &Vec<LocalRepo>,
    github_team_prefix: &String,
) -> bool {
    for local_repo in local_repos {
        if local_repo.name == remote_repo.name.replace(github_team_prefix.as_str(), "") {
            return true;
        }
    }
    return false;
}

#[tokio::main]
async fn main() {
    let base_repo_dir = std::env::var("REPO_ROOT_DIR")
        .expect("REPO_ROOT_DIR not set. It has to point to the directory with all repos.");
    let token = std::env::var("GITHUB_TOKEN")
        .expect("GITHUB_TOKEN not set. Its needed to list all github repos.");
    let github_team_repo_url = std::env::var("GITHUB_TEAM_REPO_URL")
        .expect("GITHUB_TEAM_REPO_URL not set. Points to github repo list. e.g. https://api.github.com/organizations/[organization_id]/team/[team_id]/repos");
    let github_team_prefix = std::env::var("GITHUB_TEAM_PREFIX")
        .expect("GITHUB_TEAM_PREFIX not set. e.g. [team_name_] When cloning this prefix would be removed. If your team does not use it, set it to empty.");

    let local_repos = list_local_repos(&base_repo_dir);
    let mut pull_handles = Vec::new();
    for local_repo in local_repos.clone() {
        let handle = tokio::task::spawn_blocking(|| {
            let result = git_pull(local_repo);
            return result;
        });
        pull_handles.push(handle);
    }

    let github_team_repos =
        list_github_team_repos(&token, &github_team_repo_url, &github_team_prefix).await;
    let new_repos: Vec<RemoteRepo> = github_team_repos
        .clone()
        .into_iter()
        .filter(|repo| !is_known_repo(&repo, &local_repos, &github_team_prefix))
        .collect();

    let mut clone_handles = Vec::new();
    for new_repo in new_repos.clone() {
        let base_repo_dir_clone = base_repo_dir.clone();
        let github_team_prefix_clone = github_team_prefix.clone();
        let handle = tokio::task::spawn_blocking(|| {
            println!("cloning {}", &new_repo.name);
            let result = git_clone(new_repo, base_repo_dir_clone, github_team_prefix_clone);
            return result;
        });
        clone_handles.push(handle);
    }

    for handle in pull_handles {
        let result = handle.await.unwrap();
        if let Err(message) = result {
            println!("{}", message);
        }
    }
    println!("Pulled {} repos.", local_repos.len());
    for handle in clone_handles {
        let result = handle.await.unwrap();
        if let Err(message) = result {
            println!("{}", message);
        }
    }
    println!("Cloned {} repos.", new_repos.len());
}
