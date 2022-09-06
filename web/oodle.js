function setupButtons() {
	let buttons = document.getElementsByClassName('edit');

	for (let edit of buttons) {
		edit.addEventListener('click', editClicked);
	}

	document.getElementById('cancel-edit').addEventListener('click', function () { clearEdit(); resetForm(); });
}
setupButtons()

let main = document.getElementsByTagName('main')[0];

let messageForm = document.getElementById("message-form");
let cancelEditLabel = document.getElementById('cancel-edit-label');
let submitButton = document.getElementById("submit");
let oodleFilename = document.getElementById("filename").value;
let messageIdInput = undefined;

let editingId = undefined;
let ghost = undefined;

function editMessage(messageId) {
	if (messageId == undefined) {
		clearEdit();
		return;
	}

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

