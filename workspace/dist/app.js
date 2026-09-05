(() => {
  "use strict";

  const invoke = window.__TAURI__.core.invoke;
  const output = document.getElementById("output");
  const health = document.getElementById("health");
  const encryption = document.getElementById("encryption");

  const fields = {
    fileName: document.getElementById("fileName"),
    booksId: document.getElementById("booksId"),
    companyName: document.getElementById("companyName"),
    actor: document.getElementById("actor")
  };

  function openRequest() {
    return {
      fileName: fields.fileName.value.trim(),
      booksId: fields.booksId.value.trim(),
      actor: fields.actor.value.trim()
    };
  }

  function show(value) {
    output.textContent = typeof value === "string"
      ? value
      : JSON.stringify(value, null, 2);
  }

  async function run(command, payload) {
    try {
      show(await invoke(command, payload));
    } catch (error) {
      show(String(error));
    }
  }

  document.getElementById("createButton").addEventListener("click", () => {
    run("books_create", {
      request: {
        fileName: fields.fileName.value.trim(),
        booksId: fields.booksId.value.trim(),
        companyName: fields.companyName.value.trim(),
        actor: fields.actor.value.trim()
      }
    });
  });

  document.getElementById("openButton").addEventListener("click", () => {
    run("books_open", { request: openRequest() });
  });

  document.getElementById("verifyButton").addEventListener("click", () => {
    run("books_verify", { request: openRequest() });
  });

  document.getElementById("balanceButton").addEventListener("click", () => {
    run("books_trial_balance", { request: openRequest() });
  });

  Promise.all([
    invoke("foundation_health"),
    invoke("production_encryption_required")
  ]).then(([foundation, required]) => {
    health.textContent = foundation;
    encryption.textContent = required ? "required" : "unexpectedly disabled";
  }).catch((error) => {
    health.textContent = "IPC error";
    encryption.textContent = "unknown";
    show(String(error));
  });
})();
