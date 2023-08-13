'use strict';
const $ = document.querySelector.bind(document);

//type Seat = 'north' | 'east' | 'south' | 'west';
const SEATS = ['north', 'east', 'south', 'west'];

let game;

window.addEventListener('load', async (e) => {
    let gameId = new URL(document.location).searchParams.get('game_id');
    if (gameId === null) {
        gameId = crypto.randomUUID();
        setupHost(gameId);
    } 
    const userId = crypto.randomUUID();
    game = new Game(gameId, userId);
    await game.join();
});

function setupHost(gameId) {
    const url = new URL(document.location);
    url.searchParams.set('game_id', gameId);
    $('#game-link').textContent = url;
    $('#game-link').href = url;
//    history.pushState({}, '', url);
    $('#host').style.display = 'block';
}

class Game {
    seatToRelative = null;
    mySeat = null;
    options = null;
    cards = null;

    constructor(gameId) {
        this.gameId = gameId;
        this.userId = null;
    }

    async join() {
        const joinURL = this.joinURL();
        const joinResponse = await fetch(joinURL, { method: 'POST' });
        if (!joinResponse.ok) {
            throw new Error('could not join the game', { cause: await joinResponse.text() });
        }
        const { userId } = await joinResponse.json();
        this.userId = userId;

        console.log(userId);

        const subscribeURL = this.apiURL('/api/subscribe');
        // FIXME: resolve join() only when the future opens or errors?
        const connection = new EventSource(subscribeURL); 
        connection.addEventListener('error', (e) => {
            console.error('subscription error', e);
        });
        connection.addEventListener('open', async (e) => await this.onJoin());
        connection.addEventListener('welcome', async ({ data }) => await this.onWelcome(JSON.parse(data)));
        connection.addEventListener('connected', async ({ data }) => await this.onConnected(JSON.parse(data)));
        connection.addEventListener('username', async ({ data }) => await this.onSetUsername(JSON.parse(data)));
        connection.addEventListener('deal', async ({ data }) => await this.onDeal(JSON.parse(data)));
        connection.addEventListener('play', async ({ data }) => await this.onPlay(JSON.parse(data)));
        connection.addEventListener('turn', async ({ data }) => await this.onTurn(JSON.parse(data)));
        connection.addEventListener('disconnected', async ({ data }) => await this.onDisconnected(JSON.parse(data)));
        connection.addEventListener('message', (e) => {
            throw new Error('received unspecified message', { cause: e });
        });
    }

    joinURL() {
        const endpoint = '/api/join';
        // FIXME: I think using the first argument of URL is wrong, because
        // we want to be robust to hosting the game at like pusoygame.com/pusoy/ or pusoygame.com/
        const url = new URL(endpoint, document.location);
        url.searchParams.set('game_id', this.gameId);
        return url;
    }

    apiURL(endpoint) {
        // FIXME: I think using the first argument of URL is wrong, because
        // we want to be robust to hosting the game at like etomicbomb.com/pusoy/ or pusoygame.com/
        const url = new URL(endpoint, document.location);
        url.searchParams.set('game_id', this.gameId);
        url.searchParams.set('user_id', this.userId);
        return url;
    }

    async onJoin() {
        console.log('made a connection');  

        $('#copy-game-link').addEventListener('click', async () => {
            const linkText = $('#game-link').href;
            await navigator.clipboard.writeText(linkText);
        });

        $('#my .username').addEventListener('input', async (e) => {
            const username = e.target.textContent;
            const url = this.apiURL('/api/username');
            const method = 'PUT';
            const headers = { 'Content-Type': 'application/json' };
            const body = JSON.stringify({ username });
            const response = await fetch(url, { method, headers, body });
        });

        $('#set-action-timer').addEventListener('input', async (e) => {
            const millis = parseInt(e.target.value, 10);
            $('#action-timer-value').textContent = `${millis/1000}s`;  
            const url = this.apiURL('/api/action-timer');
            const method = 'PUT';
            const headers = { 'Content-Type': 'application/json' };
            const body = JSON.stringify({ millis });
            const result = await fetch(url, { method, headers, body });
        });

        $('#play-cards-button').addEventListener('click', async (e) => {
            const value = $('#play-cards').value;
            const cards = value === '' ? [] : value.split(',');
            console.log('playing', cards);
            const url = this.apiURL('/api/play');
            const method = 'POST';
            const headers = { 'Content-Type': 'application/json' };
            const body = JSON.stringify({ cards });
            const result = await fetch(url, { method, headers, body });
        });

        $('#start-game-button').addEventListener('click', async (e) => {
            const url = this.apiURL('/api/start-game');
            const method = 'POST';
            const headers = { 'Content-Type': 'application/json' };
            const result = await fetch(url, { method, headers });
        });
    }
    
    async onWelcome({ seat }) {
        console.log('welcome', seat);
        this.mySeat = seat;
        const relatives = ['my', 'left', 'across', 'right'];
        const offset = SEATS.indexOf(this.mySeat);
        this.seatToRelative = {};
        for (let i = 0; i<SEATS.length; i++) {
            this.seatToRelative[SEATS[i]] = relatives[(i-offset+4)%relatives.length];
        }
        console.log('relative', this.seatToRelative);
    }

    async onConnected({ seat }) {
        console.log('connected', seat);
        const relative = this.seatToRelative[seat];
        $(`#${relative} .avatar`).textContent = 'human';
    }

    async onSetUsername({ seat, username }) {
        console.log('username', seat, username);
        const relative = this.seatToRelative[seat];
        $(`#${relative} .username`).textContent = username;
    }

    async onDeal({ cards }) {
        console.log('deal', cards);
        this.cards = cards;
        $(`#my .cards`).textContent = this.cards;
        $(`#my .load`).textContent = this.cards.length;
    }

    async onPlay({ seat, load, cards, pass }) {
        console.log('play', seat, load, cards);
        const relative = this.seatToRelative[seat];
        $(`#${relative} .action-timer .progress`).style.removeProperty('animation');
        $(`#${relative} .load`).textContent = load;
        $(`#${relative} .turn`).textContent = '';
        $(`#${relative} .control`).textContent = '';
        if (relative === 'my') {
            this.cards = this.cards.filter(card => !cards.includes(card));
            $(`#my .cards`).textContent = this.cards;
            $(`#my .load`).textContent = this.cards.length;
        }
        if (pass) {
            $(`#${relative} .passed`).textContent = 'passed';
        } else {
            $(`#table .cards`).textContent = cards;
        }
        if (load === 0) {
            $(`#${relative} .win`).textContent = 'win';
        }
    }

    async onTurn({ seat, timer, control }) {
        console.log('turn', seat, timer);
        const relative = this.seatToRelative[seat];
        $(`#${relative} .turn`).textContent = 'turn';
        $(`#${relative} .control`).textContent = control ? 'control' : '';
        $(`#${relative} .passed`).textContent = '';
        if (timer !== null) {
            $(`#${relative} .action-timer .progress`).style.animation = `${timer}ms linear 0s action-progress`;
        }
    }

    async onDisconnected({ seat }) {
        console.log('disconnected', seat);
        const relative = this.seatToRelative[seat];
        $(`#${relative} .avatar`).textContent = 'bot';
    }

}

function throttle(period, callback) {
    let last = 0;
    return async (...args) => {
        const now = Date.now();
        if (now - last < period) return;
        last = now;
        await callback(args);
    };
}



//$('#fullscreen').addEventListener('click', async () => {
//    try {
//        await document.documentElement
//            .requestFullscreen({ navigationUI: 'hide' })
//    } catch (e) {
//        $('h1').textContent = JSON.stringify(e);
//        console.log(e);
//    }
//});
