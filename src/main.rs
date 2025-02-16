use langchain_rust::{
    chain::{Chain, LLMChainBuilder},
    fmt_message, fmt_placeholder, fmt_template,
    language_models::llm::LLM,
    llm::openai::{OpenAI, OpenAIModel, OpenAIConfig},
    message_formatter,
    prompt::HumanMessagePromptTemplate,
    prompt_args,
    schemas::messages::Message,
    template_fstring,
};
use dotenv::dotenv; 
use std::{env, fs, error::Error};
use git2::Repository;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    dotenv().ok();
    let openai_api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");
    let mut args = env::args().skip(1);
    let repo_name = args.next().expect("Please provide a repository name (e.g., user/repo)");
    let repo_url = format!("https://github.com/{}.git", repo_name);
    let file_path = args.next().expect("Please provide a file path");
    let modification_text = args.next().expect("Please provide a modification text");
    

    let mut branch_name: Option<String> = None;
    while let Some(arg) = args.next() {
        if arg == "--branch" {
            branch_name = args.next();
        } else if arg == "-b" {
            branch_name = args.next();
        }
    }

    let local_repo_path = format!("tmp/{}", repo_name);
    println!("Cloning repository at URL: {}", repo_url);
    println!("Local repository path: {}", local_repo_path);
    let repo = Repository::clone(&repo_url, &local_repo_path)
        .unwrap_or_else(|_| panic!("Could not clone repository at URL: {}", repo_url));


    if let Some(ref branch) = branch_name {
        let branch_ref = format!("refs/heads/{}", branch);
        match repo.find_reference(&branch_ref) {
            Ok(_) => {
                repo.set_head(&branch_ref)?;
            }
            Err(_) => {
                let head = repo.head()?;
                let head_commit = head.peel_to_commit()?;
                repo.branch(&branch, &head_commit, false)?;
                repo.set_head(&branch_ref)?;
            }
        }
        repo.checkout_head(None)?;
    }

    let full_file_path = format!("{}/{}", local_repo_path, file_path);

    let original_content = fs::read_to_string(&full_file_path)
        .unwrap_or_else(|_| panic!("Could not read file at path: {}", full_file_path));

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
            // remove the cloned repository
            fs::remove_dir_all(&local_repo_path)
                .unwrap_or_else(|_| panic!("Could not remove directory at path: {}", local_repo_path));
        }
        Err(e) => panic!("Error invoking LLMChain: {:?}", e),
    }

    Ok(())
}
