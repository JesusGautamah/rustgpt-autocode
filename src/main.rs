use langchain_rust::{
    chain::{Chain, LLMChainBuilder},
    fmt_message, fmt_placeholder, fmt_template,
    language_models::llm::LLM,
    llm::openai::{OpenAI, OpenAIModel},
    message_formatter,
    prompt::HumanMessagePromptTemplate,
    prompt_args,
    schemas::messages::Message,
    template_fstring,
};

#[tokio::main]
async fn main() {
    let open_ai = OpenAI::default().with_model(OpenAIModel::Gpt4oMini.to_string());
    let resp = open_ai.invoke("What is rust").await.unwrap();
    println!("{}", resp);

    let prompt = message_formatter![
        fmt_message!(Message::new_system_message(
          "You are world class thechnical documentation writer."
        )),
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
            "input" => "Who is the writer of 20.000 Leagues Under the Sea, and what is my name?",
            "history" => vec![
                Message::new_human_message("My name is: luis"),
                Message::new_ai_message("Hi luis"),
            ],
        })
        .await
    {
        Ok(result) => {
            println!("Result: {:?}", result);
        }
        Err(e) => panic!("Error invoking LLMChain: {:?}", e),
    }
}
