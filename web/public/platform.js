// Platform SDK failures never prevent the Rust/WASM game from starting.
(() => {
  const load = (src) => new Promise((resolve, reject) => {
    const script = document.createElement('script');
    script.src = src; script.async = true; script.onload = resolve; script.onerror = reject;
    document.head.appendChild(script);
  });
  let ready = false;
  let telegram;
  const sync = () => {
    if (!telegram) return;
    const root = document.documentElement;
    if (telegram.viewportStableHeight > 0) root.style.setProperty('--app-height', `${telegram.viewportStableHeight}px`);
    const safe = telegram.contentSafeAreaInset || telegram.safeAreaInset || {};
    root.style.setProperty('--platform-top', `${safe.top || 0}px`);
    root.style.setProperty('--platform-bottom', `${safe.bottom || 0}px`);
  };
  window.addEventListener('spider-ready', () => { ready = true; telegram?.ready(); sync(); });
  const params = new URLSearchParams(location.search);
  const tgLaunch = new URLSearchParams(location.hash.slice(1)).has('tgWebAppData') || params.has('tgWebAppPlatform');
  if (tgLaunch) {
    load('https://telegram.org/js/telegram-web-app.js').then(() => {
      telegram = window.Telegram?.WebApp;
      if (!telegram) return;
      telegram.expand(); telegram.setHeaderColor?.('#003c2a'); telegram.setBackgroundColor?.('#003c2a');
      ['viewportChanged', 'safeAreaChanged', 'contentSafeAreaChanged'].forEach(e => telegram.onEvent(e, sync));
      if (ready) telegram.ready(); sync();
      window.dispatchEvent?.(new Event('spider-platform-ready'));
    }).catch(() => console.info('Telegram SDK unavailable; browser mode'));
  }
  if (params.has('vk_app_id')) {
    load('https://unpkg.com/@vkontakte/vk-bridge@2.15.11/dist/browser.min.js').then(() => {
      const bridge = window.vkBridge?.default || window.vkBridge;
      bridge?.subscribe?.(({ detail }) => {
        const insets = detail?.data?.insets;
        if (!insets) return;
        document.documentElement.style.setProperty('--platform-top', `${insets.top || 0}px`);
        document.documentElement.style.setProperty('--platform-bottom', `${insets.bottom || 0}px`);
      });
      return bridge?.send('VKWebAppInit');
    }).catch(() => console.info('VK Bridge unavailable; browser mode'));
  }
})();
