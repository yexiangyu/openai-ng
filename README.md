# openai-ng

OpenAI compatible api sdk for `rust` and `tokio`, with lot's of `builder`.

## Tested LLM API Service

| LLM Vendor | Chat | Chat Stream | File | Image Generation |
| ---------- | ---- | ----------- | ---- | ---------------- |
| Kimi       | ✅   | ✅          | ✅   | ❌               |
| Stepfun    | ✅   | ✅          | ✅   | ✅               |
| OpenAI     | ﹖   | ﹖          | ﹖   | ﹖               |

- ✅ Tested and passed
- ❌ Not available
- ﹖ Not tested

## Getting started

```rust
use openai_ng as openai;

let base_url = "https://api.stepfun.com";
let version = "v1"
let model_name = "step-1-8k";
let key = "you api service key";

// to build a client
let cli = openai::client::Client::builder()
            .with_base_url(base_url)?
            .with_version(version)?
            .with_authenticator(openai::auth::Bearer::new(key))?
            .build()?;

// to build a chat request
let req = openai::proto::chat::ChatCompletionRequest::builder()
            // select model name
            .with_model(model_name) 
            // api call messages
            .with_messages(vec![
                openai::proto::chat::Message::builder()
                    .with_role(Role::system)
                    .with_content("you are a nice and graceful llm, you are kind, helpful, patient and will not voilate rules to provide any non-proper content.")
                    .build(),
                openai::proto::chat::Message::builder()
                    .with_role(Role::user)
                    .with_content("calculate 1921.23 + 42.00")
                    .build(),
            ])
            // add tool calls
            .with_tool(
                openai::proto::tool::Function::builder()
                    .with_name("add_number")
                    .with_description("add 2 float number")
                    .with_parameters(
                        openai::proto::tool::Parameters::builder()
                            .add_property(
                                "a",
                                openai::proto::tool::ParameterProperty::builder()
                                    .with_description("1 of 2 numbers")
                                    .with_type(openai::proto::tool::ParameterType::number)
                                    .build()?,
                            )
                            .add_required("a")
                            .add_property(
                                "b",
                                openai::proto::tool::ParameterProperty::builder()
                                    .with_description("2 of 2 numbers")
                                    .with_type(openai::proto::tool::ParameterType::number)
                                    .build()?,
                            )
                            .add_required("b")
                            .build()?,
                    )
                    .build()?
            )
            .build()?;

let rep = req.call(&client, None).await?;
```
