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
use openai_ng::prelude::*;

// all api call should be run in `tokio` runtime
#[tokio::main]
async fn main() -> Result<()> {
    // build a client
    let client = Client::builder()
        .with_base_url("https://api.stepfun.com")?
        .with_key("you api key")?
        .with_version("v1")?
        .build()?;

    // build a request
    let req = ChatCompletionRequest::builder()
        .with_model("step-1-8k")
        .with_messages([
            Message::builder()
                .with_role(Role::system)
                .with_content("you are a good llm model")
                .build(),
            Message::builder()
                .with_role(Role::user)
                .with_content("calculate 1921.23 + 42.00")
                .build(),
        ])
        .with_tools([Function::builder()
            .with_name("add_number")
            .with_description("add two numbers")
            .with_parameters(
                Parameters::builder()
                    .add_property(
                        "a",
                        ParameterProperty::builder()
                            .with_description("number 1 in 2 numbers")
                            .with_type(ParameterType::number)
                            .build()?,
                        true,
                    )
                    .add_property(
                        "b",
                        ParameterProperty::builder()
                            .with_description("number 2 in 2 numbers")
                            .with_type(ParameterType::number)
                            .build()?,
                        true,
                    )
                    .build()?,
            )
            .build()?])
        .with_stream(false) // if true, the response will be a stream
        .build()?;

    // call request
    let res = req.call(&client, None).await?;

    // base on with_stream, the rep will be different
    let rep = match res {
        // will return result at once
        ChatCompletionResult::Response(rep) => rep,
        // will return a async receiver of ChatCompletionStreamData
        ChatCompletionResult::Delta(mut rx) => {
            let mut rep_total = ChatCompletionResponse::default();
            while let Some(res) = rx.recv().await {
                match res {
                    Ok(rep) => {
                        rep_total.merge_delta(rep);
                    }
                    Err(e) => {
                        error!("failed to recv rep: {:?}", e);
                        break;
                    }
                }
            }
            rep_total
        }
    };

    // log and print result
    for l in serde_json::to_string_pretty(&rep)?.lines() {
        info!("FINAL REP: {}", l);
    }

    Ok(())
}
```

about sample code will return `rep` like this: 

```json
{
  "id": "7cd553bc5e20f38a411e5a9935da2e2c.94a60989e4b9393614b28ea9d307fc11",
  "object": "chat.completion",
  "created": 1724078352,
  "model": "step-1-8k",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "",
        "tool_calls": [
          {
            "id": "call_Glo0ppwORpmSjaIJGCdG-g",
            "type": "function",
            "function": {
              "name": "add_number",
              "arguments": "{\"a\": 1921.23, \"b\": 42.0}"
            }
          }
        ]
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "completion_tokens": 30,
    "prompt_tokens": 143,
    "total_tokens": 173
  }
}
```
