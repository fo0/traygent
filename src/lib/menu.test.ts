import { describe, expect, it } from 'vitest';
import { Presses, menuIndex, openingFocus } from './menu';

describe('menuIndex', () => {
  it('moves down and up one item and wraps at both ends', () => {
    expect(menuIndex('ArrowDown', 0, 4)).toBe(1);
    expect(menuIndex('ArrowDown', 3, 4)).toBe(0);
    expect(menuIndex('ArrowUp', 2, 4)).toBe(1);
    expect(menuIndex('ArrowUp', 0, 4)).toBe(3);
  });
  it('enters the list from either end while no item has focus', () => {
    expect(menuIndex('ArrowDown', -1, 4)).toBe(0);
    expect(menuIndex('ArrowUp', -1, 4)).toBe(3);
  });
  it('jumps to the first and last item with Home/End and PageUp/PageDown', () => {
    expect(menuIndex('Home', 2, 4)).toBe(0);
    expect(menuIndex('PageUp', 2, 4)).toBe(0);
    expect(menuIndex('End', 1, 4)).toBe(3);
    expect(menuIndex('PageDown', 1, 4)).toBe(3);
  });
  it('leaves other keys and empty menus alone', () => {
    expect(menuIndex('Enter', 1, 4)).toBeNull();
    expect(menuIndex('a', 1, 4)).toBeNull();
    expect(menuIndex('ArrowLeft', 1, 4)).toBeNull();
    expect(menuIndex('ArrowDown', -1, 0)).toBeNull();
  });
});

describe('openingFocus', () => {
  it('opens on the first item with ArrowDown and on the last with ArrowUp', () => {
    expect(openingFocus('ArrowDown')).toBe('first');
    expect(openingFocus('ArrowUp')).toBe('last');
  });
  it('leaves Enter and Space to the button click', () => {
    expect(openingFocus('Enter')).toBeNull();
    expect(openingFocus(' ')).toBeNull();
  });
});

describe('Presses', () => {
  const click = { detail: 1 };
  it('the click of a press that closed a popup does nothing else', () => {
    const presses = new Presses();
    presses.press(10);
    presses.close(10);
    expect(presses.closedBy(click)).toBe(true);
  });
  it('the next press acts again', () => {
    const presses = new Presses();
    presses.close(10);
    presses.press(20);
    expect(presses.closedBy(click)).toBe(false);
  });
  it('a popup closed after the tracker saw the press still counts', () => {
    const presses = new Presses();
    presses.close(10);
    // The document's own listener runs for the same press afterwards.
    presses.press(10);
    expect(presses.closedBy(click)).toBe(true);
  });
  it('without a time, the press under way closed the popup (a blur)', () => {
    const presses = new Presses();
    presses.press(30);
    presses.close();
    expect(presses.closedBy(click)).toBe(true);
  });
  it('a click from the keyboard (detail 0) is never refused', () => {
    const presses = new Presses();
    presses.close(10);
    expect(presses.closedBy({ detail: 0 })).toBe(false);
  });
  it('a press that closed nothing acts', () => {
    const presses = new Presses();
    presses.press(10);
    expect(presses.closedBy(click)).toBe(false);
  });
});
