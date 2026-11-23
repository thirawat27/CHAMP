//! MCP resources (read-only context a client can attach) and prompts
//! (ready-made workflows a user can pick in their agent).

use super::extended;
use super::handlers::{
    current_statuses, environment_json, list_projects_json, read_logs_json, status_json,
};
use super::Context;
use crate::process::ServiceType;
use serde_json::{json, Value};

const JSON: &str = "application/json";

pub(super) fn list_resources() -> Value {
    let entry = |uri: &str, name: &str, description: &str| json!({ "uri": uri, "name": name, "description": description, "mimeType": JSON });
    json!({ "resources": [
        entry("champ://status", "Stack status", "Services, ports, URLs, and errors."),
        entry("champ://environment", "Environment", "Database, PHP, URLs, add-ons, and folders."),
        entry("champ://projects", "Projects", "Project folders with URLs and frameworks."),
        entry("champ://addons", "Add-ons", "Add-on services and their PHP variables."),
    ] })
}

pub(super) fn list_templates() -> Value {
    json!({ "resourceTemplates": [
        { "uriTemplate": "champ://logs/{service}", "name": "Service log", "description": "Last 200 lines of caddy, php-fpm, mysql, or postgresql logs.", "mimeType": JSON },
        { "uriTemplate": "champ://projects/{name}", "name": "Project", "description": "Framework, PHP requirement, and recommended env profile of one project.", "mimeType": JSON },
        { "uriTemplate": "champ://env/{profile}", "name": "Env profile", "description": "Configuration lines for laravel, symfony, wordpress, codeigniter, cakephp, dsn, or champ.", "mimeType": JSON },
    ] })
}

pub(super) async fn read_resource(ctx: &Context, uri: &str) -> Result<Value, String> {
    let path = uri
        .strip_prefix("champ://")
        .ok_or_else(|| format!("Unknown resource `{uri}`."))?;
    let value = match path.split_once('/') {
        None => match path {
            "status" => status_json(&current_statuses(&ctx.app).await?),
            "environment" => environment_json(&ctx.app).await?,
            "projects" => list_projects_json()?,
            "addons" => extended::addons_json(),
            _ => return Err(format!("Unknown resource `{uri}`.")),
        },
        Some(("logs", service)) => {
            let service: ServiceType = serde_json::from_value(json!(service))
                .map_err(|_| format!("Unknown service `{service}` in `{uri}`."))?;
            read_logs_json(service, 200)?
        }
        Some(("projects", name)) => extended::get_project(&json!({ "name": name })).await?,
        Some(("env", profile)) => {
            extended::get_env_profiles(&json!({ "profile": profile })).await?
        }
        _ => return Err(format!("Unknown resource `{uri}`.")),
    };
    Ok(json!({ "contents": [{
        "uri": uri,
        "mimeType": JSON,
        "text": serde_json::to_string_pretty(&value).unwrap_or_default(),
    }] }))
}

struct Prompt {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    argument: Option<(&'static str, &'static str, bool)>,
}

const PROMPTS: [Prompt; 3] = [
    Prompt {
        name: "connect_project",
        title: "Connect a project to CHAMP",
        description: "Detect the framework, create its database, and write its configuration.",
        argument: Some(("project", "Project folder name", true)),
    },
    Prompt {
        name: "diagnose_problem",
        title: "Find out why something fails",
        description: "Check the stack, run Log Doctor, and read the logs before changing anything.",
        argument: Some((
            "service",
            "caddy, php-fpm, mysql, postgresql, or an add-on (optional)",
            false,
        )),
    },
    Prompt {
        name: "test_email_flow",
        title: "Test an email flow",
        description: "Trigger a sign-up or reset email and verify it in Mailpit.",
        argument: Some(("project", "Project folder name (optional)", false)),
    },
];

pub(super) fn list_prompts() -> Value {
    json!({ "prompts": PROMPTS.iter().map(|prompt| json!({
        "name": prompt.name,
        "title": prompt.title,
        "description": prompt.description,
        "arguments": prompt.argument.map(|(name, description, required)| vec![json!({ "name": name, "description": description, "required": required })]).unwrap_or_default(),
    })).collect::<Vec<_>>() })
}

pub(super) fn get_prompt(name: &str, args: &Value) -> Result<Value, String> {
    let prompt = PROMPTS
        .iter()
        .find(|prompt| prompt.name == name)
        .ok_or_else(|| format!("Unknown prompt `{name}`."))?;
    let arg = |key: &str| {
        args[key]
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    };
    let text = match name {
        "connect_project" => {
            let project = arg("project").ok_or("`project` is required.")?;
            format!(
                "Connect the project \"{project}\" to CHAMP.\n\
                 1. Call get_project with name \"{project}\" to learn its framework, PHP requirement, and recommended env profile.\n\
                 2. If PHP does not satisfy composer.json, call switch_php_version.\n\
                 3. Call create_database with its suggested_database name.\n\
                 4. Write the recommended profile's lines into its target file (merge with existing keys; never delete the user's values). Use get_env_profiles if another profile fits better.\n\
                 5. Follow each advice item (composer install, copying .env.example, migrations).\n\
                 6. Open the project URL and confirm it loads; on errors call diagnose_service for php-fpm."
            )
        }
        "diagnose_problem" => {
            let target = arg("service").unwrap_or("the failing service");
            format!(
                "Find out why {target} fails before changing anything.\n\
                 1. Call get_status (and get_addons for add-ons) to see which service is in the error state.\n\
                 2. For a core service call diagnose_service, then read_logs; for an add-on call read_addon_log.\n\
                 3. Explain the cause with the matching log line, then propose the smallest fix. Ask before restarting the stack."
            )
        }
        _ => {
            let project = arg("project")
                .map(|project| format!(" in the project \"{project}\""))
                .unwrap_or_default();
            format!(
                "Test an email flow{project} with Mailpit (mail is caught, never delivered).\n\
                 1. Call get_addons; if Mailpit is off, call set_addon_enabled with addon \"mailpit\" and enabled true.\n\
                 2. Make sure the app's mail settings point at Mailpit (get_env_profiles shows the lines for its framework; plain PHP mail() already goes there).\n\
                 3. Trigger the flow (sign-up, password reset, notification).\n\
                 4. Call list_mail, then read_mail for the message; check recipient, subject, and body.\n\
                 5. Open the verification or reset link from read_mail's links and confirm it works."
            )
        }
    };
    Ok(json!({
        "description": prompt.description,
        "messages": [{ "role": "user", "content": { "type": "text", "text": text } }],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompts_render_and_require_their_arguments() {
        assert_eq!(
            list_prompts()["prompts"].as_array().unwrap().len(),
            PROMPTS.len()
        );
        assert!(get_prompt("connect_project", &json!({})).is_err());
        let prompt = get_prompt("connect_project", &json!({ "project": "shop" })).unwrap();
        assert!(prompt["messages"][0]["content"]["text"]
            .as_str()
            .unwrap()
            .contains("get_project with name \"shop\""));
        assert!(get_prompt("test_email_flow", &json!({})).is_ok());
        assert!(get_prompt("nope", &json!({})).is_err());
    }

    #[test]
    fn resources_and_templates_use_the_champ_scheme() {
        for resource in list_resources()["resources"].as_array().unwrap() {
            assert!(resource["uri"].as_str().unwrap().starts_with("champ://"));
        }
        for template in list_templates()["resourceTemplates"].as_array().unwrap() {
            assert!(template["uriTemplate"]
                .as_str()
                .unwrap()
                .starts_with("champ://"));
        }
    }
}
