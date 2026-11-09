import type {
  DisplayCapabilities,
  DisplayScreen,
  DisplayService,
  DisplayTarget,
} from '../slideShow';
import type { MessageBus } from '../types';
import type { Invoke } from './index';

/**
 * Audience screen on another monitor of the desktop app: Tauri lists the
 * monitors (`list_displays`) and opens a second window full screen on the one
 * chosen (`open_window_on`); messages go over the app's Tauri event channel.
 */
export class DesktopDisplays implements DisplayService {
  constructor(private readonly call: Invoke) {}

  capabilities(): DisplayCapabilities {
    return { screens: true, presentation: false, window: true };
  }

  async screens(): Promise<DisplayScreen[]> {
    try {
      return await this.call<DisplayScreen[]>('list_displays');
    } catch {
      return [];
    }
  }

  async open(query: string, target: DisplayTarget): Promise<MessageBus | null> {
    const q = query.includes('?') ? query.slice(query.indexOf('?') + 1) : query;
    await this.call<string>('open_window_on', {
      query: q,
      display: target.kind === 'screen' ? Number(target.id) : null,
    });
    return null;
  }
}
