const colors = require('tailwindcss/colors');

module.exports = {
    content: [
        './ao3fti-command-serve/templates/**/*.html',
    ],
    darkMode: "media",
    theme: {
        extend: {
            colors: {
                gray: colors.gray,
                // https://breezezin.github.io/tailwind-color-palettes/
                'blue-6': {
                    100: '#E3F2FD',
                    200: '#BBDEFB',
                    300: '#90CAF9',
                    400: '#64B5F6',
                    500: '#42A5F5',
                    600: '#2196F3',
                    700: '#1E88E5',
                    800: '#1565C0',
                    900: '#0D47A1'
                }
            },
        },
    },
    variants: {},
    plugins: [
        require('@tailwindcss/forms'),
    ],
};
