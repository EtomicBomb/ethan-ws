

window.addEventListener('load', async () => {
    document.querySelector('#fullscreen').addEventListener('click', async () => {
        try {
            await document.documentElement
                .requestFullscreen({ navigationUI: 'hide' })
        } catch (e) {
            document.querySelector('h1').innerText = JSON.stringify(e);
            console.log(e);

        }
    });


});
