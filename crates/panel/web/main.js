(function () {
  var ws = null;
  var wsReady = false;
  var rpcId = 1;
  var sensorData = new Map();
  var lastSeq = 0;
  var jsFpsFrames = 0;
  var jsFpsTick = performance.now();
  var imageDataCache = null;

  var canvas = document.getElementById("bevy-canvas");
  var ctx = canvas.getContext("2d");
  var panel = document.getElementById("panel");
  var statusEl = document.getElementById("status");
  var jsFpsEl = document.getElementById("js-fps-display");
  var rendererFpsEl = document.getElementById("renderer-fps-display");
  var resolutionEl = document.getElementById("resolution-display");
  var clockEl = document.getElementById("clock-display");

  function setStatus(text, connected) {
    statusEl.textContent = text;
    statusEl.style.color = connected ? "#8cf5af" : "rgba(245,248,255,0.65)";
  }

  function rpc(method, params) {
    if (!wsReady) {
      return;
    }
    ws.send(JSON.stringify({ jsonrpc: "2.0", id: rpcId++, method: method, params: params || {} }));
  }

  function renderCards() {
    var ids = ["cube_0", "cube_1", "cube_2", "cube_3", "cube_4", "cube_5"];
    var html = ids
      .map(function (id) {
        var item = sensorData.get(id);
        if (!item) {
          return '<div class="card"><h4>' + id + '</h4><div class="kv"><span>状态</span><span>等待数据...</span></div></div>';
        }
        return (
          '<div class="card"><h4>' + id + '</h4>' +
          '<div class="kv"><span>温度</span><span>' + Number(item.temperature).toFixed(1) + ' C</span></div>' +
          '<div class="kv"><span>湿度</span><span>' + Number(item.humidity).toFixed(1) + ' %</span></div>' +
          '<div class="kv"><span>时间</span><span>' + new Date(item.timestamp).toLocaleTimeString() + '</span></div>' +
          '</div>'
        );
      })
      .join("");

    panel.innerHTML = html;
  }

  function connectWs() {
    ws = new WebSocket("ws://127.0.0.1:18742/ws");

    ws.onopen = function () {
      wsReady = true;
      setStatus("● 已连接", true);
    };

    ws.onclose = function () {
      wsReady = false;
      setStatus("○ 未连接", false);
      setTimeout(connectWs, 1000);
    };

    ws.onerror = function () {
      ws.close();
    };

    ws.onmessage = function (event) {
      var msg;
      try {
        msg = JSON.parse(event.data);
      } catch (_e) {
        return;
      }

      if (msg.method === "sensor.snapshot" && msg.params && Array.isArray(msg.params.cubes)) {
        msg.params.cubes.forEach(function (cube) {
          sensorData.set(cube.id, cube);
        });
        renderCards();
        return;
      }

      if (msg.method === "sensor.update" && msg.params) {
        sensorData.set(msg.params.cube_id, {
          id: msg.params.cube_id,
          temperature: msg.params.temperature,
          humidity: msg.params.humidity,
          timestamp: msg.params.timestamp,
        });
        renderCards();
        return;
      }

      if (msg.method === "renderer.fps" && msg.params) {
        rendererFpsEl.textContent = "Bevy: " + Number(msg.params.fps || 0).toFixed(1);
      }
    };
  }

  function fitCanvasCss() {
    var w = Math.max(1, Math.round(canvas.clientWidth));
    var h = Math.max(1, Math.round(canvas.clientHeight));
    if (window.ipc) {
      // Keep transport resolution in CSS pixels to improve JS-side frame rate.
      var dpr = 1;
      window.ipc.postMessage(JSON.stringify({ resize: { width: w, height: h, dpr: dpr } }));
    }
  }

  new ResizeObserver(fitCanvasCss).observe(canvas);
  window.addEventListener("resize", fitCanvasCss);

  function setupResolutionToolbar() {
    document.querySelectorAll("#res-toolbar button").forEach(function (btn) {
      btn.addEventListener("click", function () {
        document.querySelectorAll("#res-toolbar button").forEach(function (b) {
          b.classList.remove("active");
        });
        btn.classList.add("active");

        var mode = btn.getAttribute("data-resolution");
        if (mode === "native") {
          rpc("display.renderResolution", { width: 0, height: 0 });
          return;
        }
        var parts = mode.split("x");
        rpc("display.renderResolution", {
          width: Number(parts[0]),
          height: Number(parts[1]),
        });
      });
    });
  }

  function updateClock() {
    clockEl.textContent = new Date().toLocaleTimeString();
  }

  function updateJsFps() {
    var now = performance.now();
    var elapsed = now - jsFpsTick;
    if (elapsed > 0 && jsFpsFrames > 0) {
      var fps = (1000 * jsFpsFrames / elapsed).toFixed(1);
      jsFpsEl.textContent = "JS: " + fps;
    }
    jsFpsFrames = 0;
    jsFpsTick = now;
  }

  function renderLoop() {
    var sab = window.__frameSab || null;
    if (sab) {
      var seq32 = new Int32Array(sab, 0, 1);
      var seq = Atomics.load(seq32, 0);
      if (seq !== lastSeq && seq > 0) {
        lastSeq = seq;
        var dv = new DataView(sab, 0, 20);
        var w = dv.getUint32(4, true);
        var h = dv.getUint32(8, true);
        if (w > 0 && h > 0) {
          if (canvas.width !== w || canvas.height !== h) {
            canvas.width = w;
            canvas.height = h;
            imageDataCache = ctx.createImageData(w, h);
            resolutionEl.textContent = w + "x" + h;
          }
          if (imageDataCache && imageDataCache.data.length === w * h * 4) {
            imageDataCache.data.set(new Uint8ClampedArray(sab, 64, w * h * 4));
            ctx.putImageData(imageDataCache, 0, 0);
            jsFpsFrames += 1;
          }
        }
      }
    }

    requestAnimationFrame(renderLoop);
  }

  function bindKeyboard() {
    window.addEventListener("keydown", function (e) {
      if (e.repeat) {
        return;
      }
      if (e.key === "w" || e.key === "W") {
        rpc("input.move", { direction: "forward" });
      } else if (e.key === "s" || e.key === "S") {
        rpc("input.move", { direction: "backward" });
      } else if (e.key === "a" || e.key === "A") {
        rpc("input.move", { direction: "left" });
      } else if (e.key === "d" || e.key === "D") {
        rpc("input.move", { direction: "right" });
      }
    });

    canvas.addEventListener("click", function (e) {
      var rect = canvas.getBoundingClientRect();
      rpc("input.pick", {
        screen_x: e.clientX - rect.left,
        screen_y: e.clientY - rect.top,
      });
    });
  }

  connectWs();
  setupResolutionToolbar();
  bindKeyboard();
  fitCanvasCss();
  updateClock();
  setInterval(updateClock, 1000);
  setInterval(updateJsFps, 1000);
  renderLoop();
})();
