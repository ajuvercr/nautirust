# nautirust

An orchestrator for the connector architecture.
This architecture defines processors that take input.
Input can be many things, mostly configuration stuff, streamReaders and streamWriters.

Readers and writers are connectors that use a channel to forward messages in some serialization (json, turtle, xml, plain).
What channel is used, usually doesn't really matter, they all have the same interface. 

Nautirust is primariy used to configure pipelines that consist of steps.
Each step corresponds to a processor and is configured with a step configuration file denoting the expected runner, runner specific arguments (source file, source function) and arguments to be configured.
Nautirust configures the parameters used for the processors per step. To do this it mostly asks the user the actual _implementations_ of the arguments.
Nautirust understands that readers and writers have to be linked up to function (the same channel configuration), this way it can guide the user in creating the correct pipeline.

After configuring the steps of a pipeline a pipeline config is generated. This config contains all the actual arguments (including channel configuration).

When Nautirust executes a configured pipeline, it executes a specific _runner_ for a specific step.
For example, if a step consists of a JS function, then a JSRunner is used to actually execute this function.
Ideally the runner starts up the channel and provides the step with a instance of a reader or writer and the configured arguments.

## Usage

Example step file (for more details see later)
```json
{
  "id": "helloWorld",
  "runnerId": "JsRunner",
  "config": {
    "jsFile": "main.js",
    "methodName": "sayHello"
  },
  "args": [
    {
      "id": "to",
      "type": "string"
    }
  ]
}
```

Generate a plan and save it to plan.json:
```
cargo run -- generate -o plan.json [...steps]
```


Execute the plan:
```
cargo run -- run plan.json
```


## Configuration

Channels and runners have to be defined, this can be done with command line arguments or a config file.
```toml
channels = "configs/channels/*.json"
runners = "configs/runners/*/runner.json"
```

This configuration file will look for any channel defined inside `configs/channels` and will
look for runners defined in `configs/runners` that have a `runner.json` file.

### Channel configuration

Example channel configuration:
```json
{
  "id": "file",
  "requiredFields": [
    "path",
    "onReplace"
  ],
  "options": [
    {"path": "test1.json", "onReplace": true},
    {"path": "test2.json", "onReplace": false},
    {"path": "test3.json"}
  ]
}
```

Here a channel is defined called `file` and specifies two fields are required: `path` and `onReplace`.
It also defines some options for the orchestrator and user to choose from.

### Runner configuration

Example runner configuration:
```json
  {
    "id": "JsRunner",
    "runnerScript": "node ./lib/index.js {config} {cwd}",
    "canUseChannel": [
      "file", "ws"
    ],
    "requiredFields": [
      "jsFile",
      "methodName"
    ]
  }
```

Here a runner called JsRunner is defined. Required fields are
- `runnerScript`: how is the runner started
- `canUseChannel`: what channels can this runner provide to the processor

When a runner is configured in a step `jsFile` and `methodName` have to be provided.


### Step configuration

Example step configuration:
```json
{
  "id": "readCsv",
  "runnerId": "JsRunner",
  "config": {
    "jsFile": "main.js",
    "methodName": "writeTo"
  },
  "args": [
    {
      "id": "config",
      "type": "any"
    },
    {
      "id": "writer",
      "type": "streamWriter",
      "targetIds": [
        "data"
      ]
    }
  ]
}
```

A step is a processor. The processor specifies that it can be executed with the `JsRunner` runner.
It also specifies what arguments have to be defined before being able to execute.

