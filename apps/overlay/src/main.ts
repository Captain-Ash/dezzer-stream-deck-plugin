import { BridgeConnection } from "./connection.js";
import { parseOptions } from "./params.js";
import { OverlayView } from "./view.js";

const options = parseOptions(window.location.search);
const view = new OverlayView(options);

const connection = new BridgeConnection(options.token, {
  onState: (state, receivedAt) => view.update(state, receivedAt),
  onConnectionChange: (connected) => view.setConnected(connected),
  onSpectrum: options.waveform ? (bands) => view.pushSpectrum(bands) : undefined,
});

connection.start();

// La progression est interpolee localement : le bridge n'a pas besoin d'emettre a 60 FPS.
const frame = () => {
  view.tick();
  window.requestAnimationFrame(frame);
};
window.requestAnimationFrame(frame);

window.addEventListener("beforeunload", () => connection.stop());
