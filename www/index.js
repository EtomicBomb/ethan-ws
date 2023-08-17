'use strict';
const $ = document.querySelector.bind(document);

//type Seat = 'north' | 'east' | 'south' | 'west';
const SEATS = ['north', 'east', 'south', 'west'];

let game;

window.addEventListener('load', async (e) => {
    let hostId = new URL(document.location).searchParams.get('hostId');
    game = await Game.create(hostId);
    await game.eventLoop();
});

class Game {
    seatToRelative = null;
    mySeat = null;
    options = null;
    cards = null;

    userId = null;
    userSecret = null;

    static async create(hostId) {
        const url = new URL('/api/join', document.location);
        if (hostId !== null) {
            url.searchParams.set('hostId', hostId);
        }
        const method = 'POST';
        const headers = { 'Accept': 'application/json-seq' };
        const response = await fetch(url, { method, headers });
        if (!response.ok) {
            throw new Error('could not connect', { cause: response.text() });
        }
        const items = response.body
            .pipeThrough(new TextDecoderStream())
            .pipeThrough(new TransformStream(new JsonSeqStream()));
        const { userId, userSecret } = JSON.parse(atob(response.headers.get('Authorization').split(' ')[1]));
        return new Game(userId, userSecret, items);
    }

    constructor(userId, userSecret, items) {
        this.userId = userId;
        this.userSecret = userSecret;
        this.items = items;
    }

    async eventLoop() {
        const dispatch = {
            welcome: this.onWelcome.bind(this),
            host: this.onHost.bind(this),
            connected: this.onConnected.bind(this),
            username: this.onUsername.bind(this),
            deal: this.onDeal.bind(this),
            play: this.onPlay.bind(this),
            turn: this.onTurn.bind(this),
            disconnected: this.onDisconnected.bind(this),
        };

        for await (const { event, data } of this.items) {
            console.log('received', event, data);
            await dispatch[event](data);
        }
    }

    apiURL(endpoint) {
        // FIXME: using the first argument of URL is wrong, because
        // we want to be robust to hosting the game at like etomicbomb.com/pusoy/ or pusoygame.com/
        const url = new URL(endpoint, document.location);
        url.searchParams.set('userId', this.gameId);
        url.searchParams.set('userSecret', this.userSecret);
        return url;
    }

    async onWelcome({ seat }) {
        this.mySeat = seat;
        const relatives = ['my', 'left', 'across', 'right'];
        const offset = SEATS.indexOf(this.mySeat);
        this.seatToRelative = {};
        for (let i = 0; i<SEATS.length; i++) {
            this.seatToRelative[SEATS[i]] = relatives[(i-offset+4)%relatives.length];
        }
        console.log('players relative', this.seatToRelative);

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
            const url = this.apiURL('/api/timer');
            const method = 'PUT';
            const headers = { 'Content-Type': 'application/json' };
            const body = JSON.stringify({ millis });
            const result = await fetch(url, { method, headers, body });
        });

        $('#start-game-button').addEventListener('click', async (e) => {
            const url = this.apiURL('/api/start');
            const method = 'POST';
            const headers = { 'Content-Type': 'application/json' };
            const result = await fetch(url, { method, headers });
        });

        $('#play-cards-button').addEventListener('input', async (e) => {
            const value = $('#play-cards').value;
            const cards = value === '' ? [] : value.split(',');
            const url = this.apiURL('/api/playable');
            const method = 'POST';
            const headers = { 'Content-Type': 'application/json' };
            const body = JSON.stringify({ cards });
            const result = await fetch(url, { method, headers, body });
            console.log(await result.text());
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

    }

    async onHost({ }) {
        const url = new URL(document.location);
        url.searchParams.set('hostId', this.userId);
        $('#game-link').textContent = url;
        $('#game-link').href = url;
        $('#host').style.display = 'block';
    }

    async onConnected({ seat }) {
        const relative = this.seatToRelative[seat];
        $(`#${relative} .avatar`).textContent = 'human';
    }

    async onUsername({ seat, username }) {
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

class JsonSeqStream {
    constructor() {
        this.buffer = '';
    }

    transform(chunk, controller) {
        this.buffer += chunk;

        for (;;) {
            let recordSeparator = this.buffer.indexOf('\x1e'); 
            if (recordSeparator === -1) {
                break;
            }
            const item = this.buffer.substring(0, recordSeparator).trim();
            this.buffer = this.buffer.slice(recordSeparator + 1);
            if (item.length > 0) {
                controller.enqueue(JSON.parse(item));
            }
        }

        try {
            controller.enqueue(JSON.parse(this.buffer));
            this.buffer = '';
        } catch {
            // maybe whatever is in the buffer is valid JSON.
            // maybe it's not. we'll finish it later either way
        }
    }

    flush(controller) {
        if (this.buffer.trim().length > 0) {
            controller.enqueue(JSON.parse(this.buffer));
        }
    }
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
