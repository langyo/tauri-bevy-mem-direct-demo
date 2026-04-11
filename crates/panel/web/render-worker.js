// Web Worker for OffscreenCanvas rendering
// Dedicated thread handles Frame polling + putImageData to avoid blocking main thread

let offscreenCanvas = null;
let ctx = null;
let sab = null;
let lastSeq = 0;
let imageDataCache = null;
let workerFpsFrames = 0;
let workerFpsTick = performance.now();

self.onmessage = function(e) {
  if (e.data.type === 'init') {
    // Receive OffscreenCanvas from main thread
    offscreenCanvas = e.data.canvas;
    ctx = offscreenCanvas.getContext('2d');
    sab = e.data.frameBuffer; // Shared ArrayBuffer
    startRenderLoop();
  } else if (e.data.type === 'fps-report-request') {
    // Main thread requesting current FPS
    var now = performance.now();
    var elapsed = now - workerFpsTick;
    if (elapsed > 0 && workerFpsFrames > 0) {
      var fps = (1000 * workerFpsFrames / elapsed).toFixed(1);
      self.postMessage({ type: 'fps', value: fps });
    }
    workerFpsFrames = 0;
    workerFpsTick = now;
  }
};

function startRenderLoop() {
  function pollAndRender() {
    if (sab && ctx && offscreenCanvas) {
      var seq32 = new Int32Array(sab, 0, 1);
      var seq = Atomics.load(seq32, 0);
      
      if (seq !== lastSeq && seq > 0) {
        lastSeq = seq;
        var dv = new DataView(sab, 0, 20);
        var w = dv.getUint32(4, true);
        var h = dv.getUint32(8, true);
        
        if (w > 0 && h > 0) {
          if (offscreenCanvas.width !== w || offscreenCanvas.height !== h) {
            offscreenCanvas.width = w;
            offscreenCanvas.height = h;
            imageDataCache = ctx.createImageData(w, h);
            // Notify main thread of resolution change
            self.postMessage({ 
              type: 'resolution-changed', 
              width: w, 
              height: h 
            });
          }
          
          if (imageDataCache && imageDataCache.data.length === w * h * 4) {
            // Copy frame data from SAB into ImageData (offset 64 is pixel data start)
            imageDataCache.data.set(new Uint8ClampedArray(sab, 64, w * h * 4));
            ctx.putImageData(imageDataCache, 0, 0);
            workerFpsFrames += 1;
          }
        }
      }
    }
    
    requestAnimationFrame(pollAndRender);
  }
  
  pollAndRender();
}
