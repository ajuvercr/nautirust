
const core = require("./core");

exports.writeTo =  function foobar(config, writer) {
    console.log("Got config hello", config);
    setTimeout(() => writer.data.push("new message"), 1000);
}

exports.read =  function foobar(config, reader) {
    console.log("Got config", config);
    reader.data.data(console.log)
}
