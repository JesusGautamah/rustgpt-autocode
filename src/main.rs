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
    let openai_api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");
    let github_token = env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");

    let matches = Command::new("Rustgpt Autocode")
        .version("0.3.0")
        .author("Jesus Gautamah <lima.jesuscc@gmail.com>")
        .about("Modifies code in a GitHub repository using OpenAI")
        .arg(Arg::new("repo")
            .help("Repository name (e.g., user/repo)")
            .required(true)
            .index(1))
        .arg(Arg::new("file")
            .help("File path in the repository")
            .required(true)
            .index(2))
        .arg(Arg::new("modification")
            .help("Modification text")
            .required(true)
            .index(3))
        .arg(Arg::new("branch")
            .short('b')
            .long("branch")
            .help("Branch name")
            .action(clap::ArgAction::Set)
            .default_value("main"))
        .get_matches();

    let repo_name = matches.get_one::<String>("repo").unwrap();
    let file_path = matches.get_one::<String>("file").unwrap();
    let modification_text = matches.get_one::<String>("modification").unwrap();
    let branch = matches.get_one::<String>("branch").unwrap();

    let octocrab = Octocrab::builder()
        .personal_token(github_token)
        .build()
        .expect("Could not create Octocrab instance");

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
        .expect("File not found")
        .content;

    let file_content = file_content.expect("File content is None");

    // Remove new line characters from the base64 content
    let file_content = file_content.replace("\n", "");

    // Check if the content is properly base64 encoded
    let decoded_content = match base64::engine::general_purpose::STANDARD.decode(&file_content) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Failed to decode base64 content: {:?}", e);
            eprintln!("Content: {:?}", file_content);
            return Err(Box::new(e) as Box<dyn Error>);
        }
    };

    let original_content = String::from_utf8(decoded_content).map_err(|e| Box::new(e) as Box<dyn Error>)?;

    let open_ai = OpenAI::default()
                 .with_config(OpenAIConfig::default()
                              .with_api_key(openai_api_key)
                 ).with_model(OpenAIModel::Gpt4oMini.to_string());

    let prompt = message_formatter![
        fmt_message!(Message::new_system_message(
          "You are an AI assistant specialized in modifying code based on user instructions.
           Do not send additional comments and always return the complete code.
           Use JSON format to retrieve the response:
           { 'content': 'your code here' }"
        )),
        fmt_placeholder!("history"),
        fmt_template!(HumanMessagePromptTemplate::new(template_fstring!(
          "{input}", "input"
        )))
    ];

    let chain = LLMChainBuilder::new()
        .prompt(prompt)
        .llm(open_ai.clone())
        .build()
        .unwrap();

    match chain
        .invoke(prompt_args! {
            "input" => original_content,
            "history" => vec![
                Message::new_human_message(modification_text),
                Message::new_ai_message("Sure, show me the code you want me to modify.")
            ],
        })
        .await
    {
        Ok(result) => {
            println!("Result: {:?}", result);
        }
        Err(e) => panic!("Error invoking LLMChain: {:?}", Box::new(e) as Box<dyn Error>),
    }

    Ok(())
}
