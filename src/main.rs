use langchain_rust::{
    chain::{Chain, LLMChainBuilder},
    fmt_message, fmt_placeholder, fmt_template,
    llm::openai::{OpenAI, OpenAIModel, OpenAIConfig},
    message_formatter,
    prompt::HumanMessagePromptTemplate,
    prompt_args,
    schemas::messages::Message,
    template_fstring,
};
use dotenv::dotenv;
use std::{env, error::Error};
use octocrab::Octocrab;
use clap::{Command, Arg};
use base64::Engine;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenv().ok();
    let matches = Command::new("Rustgpt Autocode")
        .version("0.6.0")
        .author("Jesus Gautamah <lima.jesuscc@gmail.com>")
        .about("Modifies code in a GitHub repository using OpenAI")
        .arg(Arg::new("repo").help("Repository name (e.g., user/repo)").required(true).index(1))
        .arg(Arg::new("file").help("File path in the repository").required(true).index(2))
        .arg(Arg::new("modification").help("Modification text").required(true).index(3))
        .arg(Arg::new("branch").short('b').long("branch").help("Branch name").default_value("main"))
        .arg(Arg::new("format").short('f').long("format").help("Output format").default_value("text"))
        .arg(Arg::new("git").long("git").help("GitHub token"))
        .arg(Arg::new("openai").long("openai").help("OpenAI API key"))
        .get_matches();

    let repo_name = matches.get_one::<String>("repo").unwrap();
    let file_path = matches.get_one::<String>("file").unwrap();
    let modification_text = matches.get_one::<String>("modification").unwrap();
    let branch = matches.get_one::<String>("branch").unwrap();
    let format = matches.get_one::<String>("format").unwrap();

    let github_token = matches.get_one::<String>("git").map_or_else(
        || env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set"),
        |v| v.clone(),
    );
    let openai_api_key = matches.get_one::<String>("openai").map_or_else(
        || env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set"),
        |v| v.clone(),
    );

    let octocrab = Octocrab::builder().personal_token(github_token).build()?;
    let repo_name_parts: Vec<&str> = repo_name.split('/').collect();
    let file_content = octocrab.repos(repo_name_parts[0], repo_name_parts[1])
        .get_content()
        .path(&*file_path)
        .r#ref(branch)
        .send()
        .await?
        .items
        .into_iter()
        .next()
        .ok_or("File not found")?
        .content;

    let file_content = file_content.ok_or("File content is None")?.replace("\n", "");
    let decoded_content = base64::engine::general_purpose::STANDARD.decode(&file_content)?;
    let original_content = String::from_utf8(decoded_content)?;

    let open_ai = OpenAI::default()
        .with_config(OpenAIConfig::default().with_api_key(openai_api_key))
        .with_model(OpenAIModel::Gpt4oMini.to_string());

    let prompt = message_formatter![
        fmt_message!(Message::new_system_message(
            "You are an AI assistant specialized in modifying code based on user instructions.\n \
            Do not send additional comments and do not use block of code.\n \
            Always return the modified code in parts and aways return the complete code, not only the alteration, using the format:\n \
            \"\"\"CODE_START\"\"\"\n\
            <your code>\n\
            \"\"\"CODE_CONTINUE\"\"\" (if there are more parts) or \"\"\"CODE_END\"\"\" (if it's the last part)."
        )),
        fmt_placeholder!("history"),
        fmt_template!(HumanMessagePromptTemplate::new(template_fstring!(
            "{input}", "input"
        )))
    ];

    let chain = LLMChainBuilder::new().prompt(prompt).llm(open_ai.clone()).build().unwrap();
    let modification_text = if format == "base64" {
        let decoded_modification_text = base64::engine::general_purpose::STANDARD.decode(modification_text.replace("\n", "").as_bytes())?;
        String::from_utf8(decoded_modification_text)?
    } else {
        modification_text.to_string()
    };

    let mut modified_content = String::new();
    let mut history = vec![
        Message::new_human_message(modification_text.clone()),
        Message::new_ai_message("Sure, show me the code you want me to modify."),
        Message::new_human_message(format!("Original file content: \n{}", original_content)),
    ];

    loop {
        match chain.invoke(prompt_args! {
            "input" => "Continue modification".to_string(),
            "history" => history.clone(),
        }).await {
            Ok(result) => {
                if result.contains("CODE_START") || result.contains("CODE_CONTINUE") || result.contains("CODE_END") {
                    modified_content.push_str(&result);
                    history.push(Message::new_ai_message(result.clone()));  

                    if result.contains("CODE_END") {
                        break;
                    } else {
                        history.push(Message::new_human_message("Continue from CODE_CONTINUE".to_string()));
                    }
                } else {
                    eprintln!("Unexpected response format, stopping.");
                    eprintln!("Response: {:?}", result);
                    return Err("Unexpected response format".into());
                }
            }
            Err(e) => {
                eprintln!("Error processing chunk: {:?}", e);
                return Err(e.into());
            }
        }
    }

    let final_content = modified_content
        .replace("\"\"\"CODE_START\"\"\"", "")
        .replace("\"\"\"CODE_CONTINUE\"\"\"", "")
        .replace("\"\"\"CODE_END\"\"\"", "");

    if format == "base64" {
        let base64_content = base64::engine::general_purpose::STANDARD.encode(final_content.as_bytes());
        println!("{}", base64_content);
    } else if format == "text" {
        println!("{}", final_content);
    }
    Ok(())
}
