import { describe, expect, it } from 'vitest';
import { Notices, SUCCESS_TIMEOUT } from './notices.svelte';

describe('Notices', () => {
  it('evicts the oldest success first and keeps warnings and errors', () => {
    const notices = new Notices();
    notices.show({ key: 'a', text: 'Ungeklärt', tone: 'warning' });
    notices.show({ key: 'b', text: 'Fehler', tone: 'danger' });
    for (const key of ['c', 'd', 'e', 'f'])
      notices.show({ key, text: `Erfolg ${key}`, tone: 'subtle' });
    expect(notices.items.map((item) => item.text)).toEqual([
      'Ungeklärt',
      'Fehler',
      'Erfolg e',
      'Erfolg f',
    ]);
  });

  it('never evicts a warning or error, and still shows the newest success', () => {
    const notices = new Notices();
    for (const key of ['a', 'b', 'c', 'd'])
      notices.show({ key, text: `Warnung ${key}`, tone: 'warning' });
    notices.show({ key: 'e', text: 'Erfolg', tone: 'subtle' });
    expect(notices.items.map((item) => item.text)).toEqual([
      'Warnung a',
      'Warnung b',
      'Warnung c',
      'Warnung d',
      'Erfolg',
    ]);
    // An error over the limit pushes out the success, never a warning.
    notices.show({ key: 'f', text: 'Fehler', tone: 'danger' });
    notices.show({ key: 'g', text: 'Neuer Erfolg', tone: 'subtle' });
    expect(notices.items.map((item) => item.text)).toEqual([
      'Warnung a',
      'Warnung b',
      'Warnung c',
      'Warnung d',
      'Fehler',
      'Neuer Erfolg',
    ]);
  });

  it('closes successes after 3 s unless they bring their own timeout; warnings and errors stay', () => {
    const notices = new Notices();
    expect(SUCCESS_TIMEOUT).toBe(3000);
    notices.show({ key: 'a', text: 'Aktualisiert', tone: 'subtle' });
    notices.show({ key: 'b', text: 'Entfernt', tone: 'subtle', timeout: 8000 });
    notices.show({ key: 'c', text: 'Warnung', tone: 'warning', timeout: 8000 });
    notices.show({ key: 'd', text: 'Fehler', tone: 'danger' });
    expect(notices.items.map((item) => item.timeout)).toEqual([
      3000,
      8000,
      undefined,
      undefined,
    ]);
  });
});
