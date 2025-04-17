(() => {
    const initializeMermaid = () => {
        const theme = document.documentElement.classList.contains('light') ? 'neutral' : 'dark';
        const mermaidConfig = {
            startOnLoad: true,
            theme: theme,
            themeVariables: {
                darkMode: theme === 'dark'
            },
            flowchart: {
                useMaxWidth: true,
                htmlLabels: true,
                curve: 'basis'
            },
            securityLevel: 'loose'
        };
        
        if (window.mermaid) {
            window.mermaid.initialize(mermaidConfig);
            window.mermaid.init();
        }
    };

    // Initialize on load
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', initializeMermaid);
    } else {
        initializeMermaid();
    }

    // Re-initialize when theme changes
    const observer = new MutationObserver((mutations) => {
        mutations.forEach((mutation) => {
            if (mutation.type === 'attributes' && mutation.attributeName === 'class') {
                initializeMermaid();
            }
        });
    });

    observer.observe(document.documentElement, {
        attributes: true,
        attributeFilter: ['class']
    });
})();