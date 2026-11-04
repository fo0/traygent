import { mount } from 'svelte';
import App from './App.svelte';
import { applyTheme, readTheme } from './lib/theme';
import './styles/index.css';

// Before mounting: the CSP forbids an inline script in index.html, and the
// native window starts hidden, so the panel never shows the other theme.
applyTheme(readTheme());

const app = mount(App, { target: document.getElementById('app')! });
export default app;
