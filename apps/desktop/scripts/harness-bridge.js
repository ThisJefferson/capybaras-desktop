// Verification bridge -- NOT SHIPPED.
//
// This is the ONE substitution BOTH interface harnesses make: `verify-acceptance.mjs`
// (the gate) and `verify-onboarding.mjs` (the first reply). The real
// `window.__TAURI__` is injected by the Rust shell's webview; these checks run
// without a desktop, so the bridge is mapped onto the harness's HTTP server
// instead. Everything ABOVE it -- index.html, app.js, app.css, tokens.css -- is
// the product's own code, unmodified, and everything the SERVER does is decided
// by the harness, not here: this file only moves bytes. For the acceptance
// harness the gate behind the server is the real sidecar.
//
// It is a classic script on purpose: it runs while the parser reaches it, which
// is BEFORE the deferred `app.js` module evaluates and reads the bridge.
//
// It cannot be inlined into the page: the app's CSP is `script-src 'self'`.
(function () {
  var listeners = Object.create(null);
  var since = 0;

  function dispatch(name, payload) {
    var list = listeners[name];
    if (!list) return;
    for (var i = 0; i < list.length; i += 1) list[i]({ payload: payload });
  }

  function poll() {
    fetch('/events?since=' + since)
      .then(function (response) { return response.json(); })
      .then(function (batch) {
        since = batch.next;
        for (var i = 0; i < batch.events.length; i += 1) {
          dispatch(batch.events[i].event, batch.events[i].payload);
        }
        setTimeout(poll, 25);
      })
      .catch(function () { setTimeout(poll, 200); });
  }

  window.__TAURI__ = {
    core: {
      invoke: function (cmd, args) {
        return fetch('/invoke', {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({ cmd: cmd, args: args || {} }),
        })
          .then(function (response) { return response.json(); })
          .then(function (body) { return body.result; });
      },
    },
    event: {
      listen: function (name, callback) {
        (listeners[name] || (listeners[name] = [])).push(callback);
        return Promise.resolve(function () {});
      },
    },
  };

  poll();
})();
