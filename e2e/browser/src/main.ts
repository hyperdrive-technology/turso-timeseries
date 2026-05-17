import { runTursoTimeseriesBrowserE2E, type E2EResult } from './runTursoTimeseriesBrowserE2E';
import { renderSandbox } from './sandbox';
import './style.css';

declare global {
  interface Window {
    runTursoTimeseriesBrowserE2E: () => Promise<E2EResult>;
  }
}

window.runTursoTimeseriesBrowserE2E = runTursoTimeseriesBrowserE2E;

const app = document.querySelector('#app');
if (app) {
  renderSandbox(app as HTMLElement);
}
