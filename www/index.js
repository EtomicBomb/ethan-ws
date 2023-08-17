'use strict';
const $ = document.querySelector.bind(document);

//type Seat = 'north' | 'east' | 'south' | 'west';
const SEATS = ['north', 'east', 'south', 'west'];

let game;

window.addEventListener('load', async (e) => {
    let sessionId = new URL(document.location).searchParams.get('sessionId');
    game = await Game.create(sessionId);
    await game.eventLoop();
});

class Game {
    cards = null;

    auth = null;
    items = null;
    seatToRelative = null;

    static async create(sessionId) {
        const url = new URL('/api/join', document.location);
        if (sessionId !== null) {
            url.searchParams.set('sessionId', sessionId);
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
        const auth = JSON.parse(atob(response.headers.get('Authorization').split(' ')[1]));
        return new Game(auth, items);
    }

    constructor(auth, items) {
        this.auth = auth;
        this.items = items;

        const relatives = ['my', 'left', 'across', 'right'];
        const offset = SEATS.indexOf(this.auth.seat);
        this.seatToRelative = {};
        for (let i = 0; i<SEATS.length; i++) {
            this.seatToRelative[SEATS[i]] = relatives[(i-offset+4)%relatives.length];
        }
        console.log('players relative', this.seatToRelative);
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

    async makeRequest(endpoint, method, body) {
        // FIXME: using the first argument of URL is wrong, because
        // we want to be robust to hosting the game at like etomicbomb.com/pusoy/ or pusoygame.com/
        const url = new URL(endpoint, document.location);
        const token = btoa(JSON.stringify(this.auth));
        const headers = { 'Authorization': `Bearer ${token}`, 'Content-Type': 'application/json' };
        body = JSON.stringify(body);
        const result = await fetch(url, { headers, method, body });
        if (!result.ok) {
            $('#error-description').textContent = await result.text();
        }
    }

    async onWelcome({ seat }) {

        $('#copy-game-link').addEventListener('click', async () => {
            const linkText = $('#game-link').href;
            await navigator.clipboard.writeText(linkText);
        });

        $('#my .username').addEventListener('input', async (e) => {
            const username = e.target.textContent;
            const response = await this.makeRequest('/api/username', 'PUT', { username });
        });

        $('#set-action-timer').addEventListener('input', async (e) => {
            const millis = parseInt(e.target.value, 10);
            $('#action-timer-value').textContent = `${Math.round(millis/1000)}s`;  
        });

        async function updateActionTimer() {
            const checked = $('#enable-action-timer').checked;
            const value = parseInt($('#set-action-timer').value, 10);
            const millis = checked ? value : null;
            const response = await this.makeRequest('/api/timer', 'PUT', { millis });
        }
        $('#enable-action-timer').addEventListener('change', updateActionTimer.bind(this));
        $('#set-action-timer').addEventListener('change', updateActionTimer.bind(this));

        $('#start-game-button').addEventListener('click', async (e) => {
            const response = await this.makeRequest('/api/start', 'POST', { });
        });

        $('#play-cards-button').addEventListener('input', async (e) => {
            const value = $('#play-cards').value;
            const cards = value === '' ? [] : value.split(',');
            const response = await this.makeRequest('/api/playable', 'POST', { cards });
        });

        $('#play-cards-button').addEventListener('click', async (e) => {
            const value = $('#play-cards').value;
            const cards = value === '' ? [] : value.split(',');
            const response = await this.makeRequest('/api/play', 'POST', { cards });
        });

    }

    async onHost({ }) {
        const url = new URL(document.location);
        url.searchParams.set('sessionId', this.auth.sessionId);
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
        $('#host').style.display = 'none';
        this.cards = cards;
        $(`#my .cards`).textContent = this.cards;
        $(`#my .load`).textContent = this.cards.length;
    }

    async onPlay({ seat, load, cards, pass }) {
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

    async onTurn({ seat, millis, control }) {
        const relative = this.seatToRelative[seat];
        $(`#${relative} .turn`).textContent = 'turn';
        $(`#${relative} .control`).textContent = control ? 'control' : '';
        $(`#${relative} .passed`).textContent = '';
        if (millis !== null) {
            $(`#${relative} .action-timer .progress`).style.animation = `${millis}ms linear 0s action-progress`;
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
