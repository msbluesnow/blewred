const dgram = require('dgram');
const client = dgram.createSocket('udp4');
const message = Buffer.from('{"obs_attached":true,"version":"1.0.0"}');

console.log("OBS Plugin Simulator Heartbeat Daemon started (UDP 51798)");

function sendHeartbeat() {
    client.send(message, 51798, '127.0.0.1', (err) => {
        if (err) console.error("Heartbeat send error:", err);
    });
}

sendHeartbeat();
setInterval(sendHeartbeat, 1000);
