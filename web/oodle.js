const postAction = "/oodle/message/create?json";
const editAction = "/oodle/message/modify?json";

const main = document.getElementsByTagName('main')[0];
const messageForm = document.getElementById("message-form");
const cancelEditLabel = document.getElementById('cancel-edit-label');
const contentTextarea = document.getElementById('content');
const submitButton = document.getElementById("submit");
const oodleFilename = document.getElementById("filename").value;

/// Look for elements with the 'edit' class in the supplied node and add the
/// proper event listener
function setupButtons(doc) {
	let buttons = doc.getElementsByClassName('edit');

	for (let edit of buttons) {
		edit.addEventListener('click', editClicked);
	}
}
setupButtons(document);
document.getElementById('cancel-edit').addEventListener('click', function () { clearEdit(); });

let messageIdInput = undefined;
let editingId = undefined;
let savedPost = undefined;

messageForm.addEventListener('submit', formSubmit);

function editClicked(e) {
	let messageId = e.target.getAttribute('message-id');
	setupEdit(messageId);
}

function clearEdit() {
	setForm(postAction);
	ghost(undefined);
}

function setupEdit(messageId) {
	if (messageId == undefined) {
		clearEdit();
		return;
	}

	editingId = messageId;

	fetch("/oodle/message/get?" + new URLSearchParams({
		filename: oodleFilename,
		id: messageId
	})).then((response) => response.json()).then((data) => {
		savedPost = contentTextarea.value;
		contentTextarea.value = data.content;
	});

	setForm(editAction, messageId);

	let messageSection = document.getElementById(`message-${messageId}`);
	ghost(messageSection);
}

let ghostedElement = undefined;
let ghostElement = undefined;

function ghost(element) {
	if (ghostedElement != undefined) {
		ghostedElement.style.display = "";
		ghostedElement.parentElement.removeChild(ghostElement);
		ghostElement = undefined;
	}

	if (element != undefined && element !== ghostedElement) {
		ghostElement = document.createElement('div');
		ghostElement.className = "ghost";
		ghostElement.style.height = element.clientHeight + "px";

		element.parentElement.insertBefore(ghostElement, element);
		element.style.display = "none";
	}

	ghostedElement = element;
}

function setForm(action, messageId) {
	messageForm.action = action;

	if (action == editAction) {
		submitButton.value = "edit";
		cancelEditLabel.style.display = "";

		messageIdInput = document.createElement('input');
		messageIdInput.type = "hidden";
		messageIdInput.name = "id";
		messageIdInput.value = messageId;
		messageForm.appendChild(messageIdInput);
	} else {
		submitButton.value = "post";
		cancelEditLabel.style.display = "none";
		messageForm.removeChild(messageIdInput);
		contentTextarea.value = savedPost;
	}
}

function formSubmit(event) {
	if (messageForm.getAttribute('action') == postAction) {
		postMessage(event);
	} else {
		editMessage(event);
	}
}

function postMessage(event) {
	event.stopPropagation();
	event.preventDefault();

	const jsonData = { 'filename': oodleFilename, 'content': document.getElementById('content').value };

	fetch(postAction, {
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

function editMessage(event) {
	event.stopPropagation();
	event.preventDefault();

	const jsonData = { 'filename': oodleFilename, 'content': document.getElementById('content').value, "id": parseInt(editingId, 10) };

	fetch(editAction, {
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
			main.insertBefore(our, ghostedElement);
			main.removeChild(ghostedElement);

			ghostedElement = our;

			clearEdit();
		})
}