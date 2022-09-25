function setupButtons(doc) {
	let buttons = doc.getElementsByClassName('edit');

	for (let edit of buttons) {
		edit.addEventListener('click', editClicked);
	}
}
setupButtons(document);
document.getElementById('cancel-edit').addEventListener('click', function () { clearEdit(); resetForm(); });

let main = document.getElementsByTagName('main')[0];

let messageForm = document.getElementById("message-form");
let cancelEditLabel = document.getElementById('cancel-edit-label');
let submitButton = document.getElementById("submit");
let oodleFilename = document.getElementById("filename").value;
let messageIdInput = undefined;

let editingId = undefined;
let ghost = undefined;

messageForm.addEventListener('submit', postMessage);

function editMessage(messageId) {
	if (messageId == undefined) {
		clearEdit();
		return;
	}

	messageForm.removeEventListener('submit', postMessage);

	editingId = messageId;

	fetch("/oodle/message/get?" + new URLSearchParams({
		filename: oodleFilename,
		id: messageId
	})).then((response) => response.json()).then((data) => {
		document.getElementById('content').value = data.content;
	});

	messageForm.action = "/oodle/message/modify";
	submitButton.value = "edit";
	cancelEditLabel.style.display = "";

	messageIdInput = document.createElement('input');
	messageIdInput.type = "hidden";
	messageIdInput.name = "id";
	messageIdInput.value = messageId;
	messageForm.appendChild(messageIdInput);

	let messageSection = document.getElementById(`message-${messageId}`);

	ghost = document.createElement('div');
	ghost.className = "ghost";
	ghost.style.height = messageSection.clientHeight + "px";

	main.insertBefore(ghost, messageSection);
	messageSection.style.display = "none";
}

function clearEdit() {
	let messageSection = document.getElementById(`message-${editingId}`);
	messageSection.style.display = "";
	main.removeChild(ghost);
	messageForm.addEventListener('submit', postMessage);
}

function resetForm() {
	messageForm.action = "/oodle/message/create";
	submitButton.value = "post";
	messageForm.removeChild(messageIdInput);
	cancelEditLabel.style.display = "none";
	document.getElementById('content').value = "";
}

function editClicked(e) {
	let messageId = e.target.getAttribute('message-id');
	editMessage(messageId);
}

/* Post message dynamically */

function postMessage(event) {
	event.stopPropagation();
	event.preventDefault();

	const jsonData = { 'filename': oodleFilename, 'content': document.getElementById('content').value };

	fetch('/oodle/message/create?json', {
		method: 'POST',
		headers: {
			'Content-Type': "application/json"
		},
		body: JSON.stringify(jsonData)
	})
		.then((response) => response.text())
		.then((body) => {
			let doc = new DOMParser().parseFromString(body, "text/html");

			setupButtons(doc);

			let our = doc.body.firstChild;
			main.insertBefore(our, document.getElementById('form-container'));

			document.getElementById('content').value = '';
		})
}