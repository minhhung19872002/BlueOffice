import { describe, expect, it } from 'vitest';
import { resolveRoute } from './routes';

describe('web routes', () => {
  it('maps canonical routes', () => {
    expect(resolveRoute('/doc')).toEqual({ app: 'doc' });
    expect(resolveRoute('/sheet')).toEqual({ app: 'sheet' });
    expect(resolveRoute('/slide/x')).toEqual({ app: 'slide' });
  });

  it('redirects legacy routes', () => {
    expect(resolveRoute('/write')).toEqual({ app: 'doc', redirect: '/doc' });
    expect(resolveRoute('/docs')).toEqual({ app: 'doc', redirect: '/doc' });
    expect(resolveRoute('/sheets')).toEqual({ app: 'sheet', redirect: '/sheet' });
    expect(resolveRoute('/slides')).toEqual({ app: 'slide', redirect: '/slide' });
    expect(resolveRoute('/')).toEqual({ app: 'doc', redirect: '/doc' });
    expect(resolveRoute('/office/write', '/office/')).toEqual({
      app: 'doc',
      redirect: '/office/doc',
    });
  });
});
