use std::ffi::OsStr;
use std::fs;
use std::{path::PathBuf, process::Command};

use reqwest::Client;
use reqwest::Url;
use reqwest::header::ACCEPT;
use reqwest::header::USER_AGENT;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct LocalRepo {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RemoteRepo {
    pub name: String,
    pub archived: bool,
    pub ssh_url: String,
}

pub fn git_clone(
    remote_repo: &RemoteRepo,
    repo_root_dir: PathBuf,
    github_team_prefix: String,
) -> Result<std::process::Output, std::io::Error> {
    let dir_without_prefix = remote_repo.name.replace(github_team_prefix.as_str(), "");
    return Command::new("git")
        .arg("clone")
        .arg(remote_repo.ssh_url.clone())
        .arg(dir_without_prefix)
        .current_dir(repo_root_dir)
        .output();
}

pub fn git_pull(local_repo: LocalRepo) -> Result<std::process::Output, std::io::Error> {
    return Command::new("git")
        .arg("pull")
        .current_dir(local_repo.path)
        .output();
}

pub fn find_new_repos(
    remote_repos: &Vec<RemoteRepo>,
    local_repos: &Vec<LocalRepo>,
    github_team_prefix: &String,
) -> Vec<RemoteRepo> {
    remote_repos
        .iter()
        .filter(|repo| !is_known_repo(repo, local_repos, github_team_prefix))
        .cloned()
        .collect()
}

pub fn find_archived_local_repos(
    remote_archived_repos: &Vec<RemoteRepo>,
    local_repos: &Vec<LocalRepo>,
    github_team_prefix: &String,
) -> Vec<RemoteRepo> {
    remote_archived_repos
        .iter()
        .filter(|repo| is_known_repo(repo, local_repos, github_team_prefix))
        .cloned()
        .collect()
}

pub fn is_known_repo(
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

pub fn is_git_repo(path: &String) -> bool {
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

pub fn list_local_repos(path: &PathBuf) -> Vec<LocalRepo> {
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

pub async fn get_repos(
    client: &Client,
    token: &String,
    page: i32,
    github_team_prefix: &String,
    github_team_repo_url: &Url,
) -> Option<Vec<RemoteRepo>> {
    let response = match client
        .get(github_team_repo_url.clone())
        .header(ACCEPT, "application/vnd.github.v3+json")
        .header(USER_AGENT, "reposync")
        .bearer_auth(token)
        .query(&[("per_page", "100"), ("page", page.to_string().as_str())])
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            println!("Request failed: {}", e);
            return None; // Return empty vector
        }
    };

    match response.json::<Vec<RemoteRepo>>().await {
        Ok(repos) => {
            let next_paged_repos = repos
                .into_iter()
                .filter(|repo| repo.name.starts_with(github_team_prefix.as_str()))
                .collect::<Vec<RemoteRepo>>();
            if next_paged_repos.is_empty() {
                return None;
            } else {
                return Some(next_paged_repos);
            }
        }

        Err(e) => {
            println!("Failed to parse JSON: {}", e);
            return None;
        }
    }
}

pub async fn get_all_repos(
    token: &String,
    github_team_prefix: &String,
    github_team_repo_url: &Url,
) -> Vec<RemoteRepo> {
    let client = Client::new();

    let mut repos: Vec<RemoteRepo> = Vec::new();
    let mut page = 1;
    while let Some(page_repos) = get_repos(
        &client,
        &token,
        page,
        github_team_prefix,
        github_team_repo_url,
    )
    .await {
        repos.extend(page_repos);
        page += 1;
    }
    return repos;
}
pub async fn list_active_github_team_repos(git_repos: Vec<RemoteRepo>) -> Vec<RemoteRepo> {
    return git_repos
        .into_iter()
        .filter(|repo| !repo.archived)
        .collect::<Vec<RemoteRepo>>();
}

pub async fn list_archived_github_team_repos(git_repos: Vec<RemoteRepo>) -> Vec<RemoteRepo> {
    return git_repos
        .into_iter()
        .filter(|repo| repo.archived)
        .collect::<Vec<RemoteRepo>>();
}
