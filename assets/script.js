const socket = new WebSocket('ws://127.0.0.1:3000/ws');

socket.addEventListener('open', function (event) {
	socket.send('hello!');
});

socket.addEventListener('message', function (event) {
	console.log('server: ', event.data);
});
