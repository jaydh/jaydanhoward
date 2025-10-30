// Web Worker for Conway's Game of Life calculations
// This worker runs WASM calculations off the main thread

let wasm = null;

// Listen for the initial setup message with the WASM path
self.onmessage = async function(e) {
  if (e.data.type === 'init') {
    try {
      // Import the WASM module
      const wasmModule = await import(e.data.wasmPath);
      await wasmModule.default();
      wasm = wasmModule;

      self.postMessage({ type: 'ready' });
    } catch (error) {
      self.postMessage({
        type: 'error',
        error: `Failed to initialize WASM: ${error.message}`
      });
    }
  } else if (e.data.type === 'calculate') {
    if (!wasm) {
      self.postMessage({
        type: 'error',
        error: 'WASM not initialized'
      });
      return;
    }

    try {
      // Call the exported WASM function
      const requestJson = JSON.stringify({
        alive_cells: e.data.aliveCells,
        grid_size: e.data.gridSize
      });

      const responseJson = wasm.life_worker_calculate(requestJson);
      const response = JSON.parse(responseJson);

      self.postMessage({
        type: 'result',
        aliveCells: response.alive_cells
      });
    } catch (error) {
      self.postMessage({
        type: 'error',
        error: `Calculation failed: ${error.message}`
      });
    }
  }
};
