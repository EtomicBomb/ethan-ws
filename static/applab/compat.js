// AppLab provides access to some functions that
// are not standard js. These can be implemented
// manually. 

const SCREEN_WIDTH = 320;
const SCREEN_HEIGHT = 450;
let compatActiveCanvas;
let compatActiveContext;
let activeScreen;

const activeScreenElement = () => {
    let screen = document.getElementById(activeScreen);
    if (!screen) {
        screen = document.getElementsByTagName('section')[0];
    }
    return screen;
};

const setScreen = (newScreen) => {
    activeScreenElement().style.display = 'none';
    activeScreen = newScreen;
    activeScreenElement().style.display = 'block';
};

const createCanvas = (id, width, height) => {
    const canvas = document.createElement('canvas');
    canvas.id = id;
    canvas.width = width;
    canvas.height = height;
    canvas.style.width = width + 'px';
    canvas.style.height = height + 'px';
    activeScreenElement().appendChild(canvas); 
};

const button = (id, label) => {
    const labelNode = document.createTextNode(label);

    const newElement = document.createElement('button');
    newElement.id = id;
    newElement.appendChild(labelNode);

    activeScreenElement().appendChild(newElement); 
};

const image = (id, url) => {
    const newElement = document.createElement('img');
    newElement.id = id;
    newElement.src = './assets/'+url;
    activeScreenElement().appendChild(newElement); 
};

const textLabel = (id, label) => {
    const labelNode = document.createTextNode(label);

    const newElement = document.createElement('p');
    newElement.id = id;
    newElement.appendChild(labelNode);

    activeScreenElement().appendChild(newElement); 
};

const stopTimedLoop = (handle) => {
    clearInterval(handle);
};

const circle = (x, y, radius) => {
    compatActiveContext.beginPath();
    compatActiveContext.arc(x, y, radius, 0, 2*Math.PI);
    compatActiveContext.fill();
};

const setStrokeWidth = (value) => {
    compatActiveContext.lineWidth = value;
};

const line = (x0, y0, x1, y1) => {
    compatActiveContext.beginPath();
    compatActiveContext.moveTo(x0, y0);
    compatActiveContext.lineTo(x1, y1);
    compatActiveContext.stroke();
};


const setProperty = (id, property, value) => {
    const element = document.getElementById(id);

    switch (property) {
        case 'text-color': 
            element.style.color = value;
            break;
        case 'options':
            const newChildren = value.map(label => {
                const labelNode = document.createTextNode(label);
                const newElement = document.createElement('option');
                newElement.appendChild(labelnode);
                return newElement;
            });
            element.replaceChildren(...newChildren);
            break;
        case 'background-color':
            element.style.backgroundColor = value;
            break;
        case 'image':
            element.style.backgroundImage = `url(./assets/${value})`;
            break;
        case 'font-size':
            element.style.fontSize = value + 'px'; 
            break;
        case 'index':
            element.selectedIndex = value; 
            break;
        default:
            throw `unknown property ${property}`;
    }
};

const getProperty = (id, property) => {
    const element = document.getElementById(id);

    switch (property) {
        case 'text-color': 
            return element.style.color;
        case 'options':
            return element.children.map(option => {
                const child = option.firstChild;
                if (child.nodeType !== Node.TEXT_NODE) {
                    throw 'should be a text node here';
                }
                return child.nodeValue;
            });
        case 'background-color':
            return element.style.backgroundColor;
        case 'image':
            return element.style.backgroundImage;
        case 'index':
            return element.selectedIndex; 
            break;
        default:
            throw `unknown property ${property}`;
    }

};

const setImageURL = (id, url) => {
    document.getElementById(id).src = './assets/'+url;
};

const setText = (id, value) => {
    const element = document.getElementById(id);
    switch (element.tagName.toLowerCase()) {
        case "input":
            element.value = value;
            return;
        default:
            const labelNode = document.createTextNode(value);
            element.replaceChildren(labelNode);
    }
};

const setNumber = (id, value) => {
    setText(id, value);
};

const getText = (id) => {
    const element = document.getElementById(id);
    switch (element.tagName.toLowerCase()) {
        case "input":
            return element.value;
        default:
            const child = element.firstChild;
            if (child === null || child.nodeType != Node.TEXT_NODE) {
                return '';
            }
            return child.textContent;
    }
};

const setChecked = (id, value) => {
    document.getElementById(id).checked = value;
};

const getChecked = (id) => {
    const element = document.getElementById(id);
    return element.checked;
};

const getNumber = (id) => {
    return +getText(id);
};

const setStyle = (id, style) => {
    document.getElementById(id).style.cssText += style;
};

const hideElement = (id) => {
    document.getElementById(id).style.visibility = 'hidden';
};

const showElement = (id) => {
    document.getElementById(id).style.visibility = 'visible';
};

const deleteElement = (id) => {
    document.getElementById(id).remove();
};

const setPosition = (id, x, y, width, height) => {
    const element = document.getElementById(id);

    if (x !== undefined) {
        element.style.left = x + 'px';
    }

    if (y !== undefined) {
        element.style.top = y + 'px';
    }

    if (width !== undefined) {
        element.style.width = width + 'px';
    }
    
    if (height !== undefined) {
        element.style.height = height + 'px';
    }
};

const setSize = (id, width, height) => {
    setPosition(id, undefined, undefined, width, height);
};

const getTime = () => {
    return new Date().getTime();
};

const createRecord = (table, record, callback, error) => {
    const options = {
        method: 'POST',
        body: JSON.stringify(record),
        headers: {
            'Content-Type': 'application/json',
        },
    };

    fetch(`./records/create/${table}`, options)
        .then((response) => {
            if (!response.ok) {
                throw `Error making request ${response.status}`;
            }
            return response.json();
        })
        .then(({ record, id }) => {
            callback({ ...record, id });
        })
        .catch((e) => {
            if (!error) {
                throw e;
            }
            error('server error:', e);
        });
};

const updateRecord = (table, { id, ...record }, callback, error) => {
    const options = {
        method: 'PATCH',
        body: JSON.stringify(record),
        headers: {
            'Content-Type': 'application/json',
        },
    };

    fetch(`./records/update/${table}/${id}`, options)
        .then((response) => {
            if (!response.ok) {
                throw `Error making request ${response.status}`;
            }
            return response.json();
        })
        .then(({ record, id }) => {
            callback({ ...record, id });
        })
        .catch((e) => {
            if (!error) {
                throw e;
            }
            error('server error:', e);
        });
};

const deleteRecord = (table, { id }, callback, error) => {
    const options = {
        method: 'DELETE',
    };

    fetch(`./records/delete/${table}/${id}`, options)
        .then((response) => {
            if (!response.ok) {
                throw `Error making request ${response.status}`;
            }
            return response.json();
        })
        .then(({ record, id }) => {
            callback({ ...record, id });
        })
        .catch((e) => {
            if (!error) {
                throw e;
            }
            error('server error:', e);
        });
};

const readRecordsId = (table, id, callback, error) => {
    const options = {
        method: 'GET',
    };

    fetch(`./records/read-id/${table}/${id}`, options)
        .then((response) => {
            if (!response.ok) {
                throw `Error making request ${response.status}`;
            }
            return response.json();
        })
        .then(({ record, id }) => {
            callback([{ ...record, id }]);
        })
        .catch((e) => {
            callback([]);
        });
};

const readRecordsQuery = (table, record, callback, error) => {
    const options = {
        method: 'POST',
        body: JSON.stringify(record),
        headers: {
            'Content-Type': 'application/json',
        },
    };

    fetch(`./records/read-query/${table}`, options)
        .catch((e) => {
            console.error(`readRecordsQuery: error making request:`, e);
            callback([]);
        })
        .then((response) => {
            if (!response.ok) {
                throw response;
            }
            return response.json();
        })
        .catch((e) => {
            console.error(`readRecordsQuery: bad request:`, e);
            callback([]);
        })
        .then((records) => {
            console.log(records);
            const flattened = records.map(({ id, record }) => { return { ...record, id }; });
            console.log(flattened);
            callback(flattened);
        });
};

const readRecords = (table, { id, ...record }, callback, error) => {
    if (id === undefined) {
        return readRecordsQuery(table, record, callback, error);
    } else {
        return readRecordsId(table, id, callback, error);
    }
};

const registerListener = (table, callback) => {
    const source = new EventSource(`./records/subscribe/${table}`);

    source.addEventListener('message', (event) => {
        const { update: { id, record }, kind } = JSON.parse(event.data);
        callback({ ...record, id }, kind);
    });

    source.addEventListener('error', (event) => {
        console.error('onRecordEvent: event source error: ', event);
    });
};

const onRecordEvent = (table, callback, all) => {
    if (all) {
        readRecordsQuery(table, {}, (response) => {
            for (record in response) {
                callback(record, 'create');
            }
            registerListener(table, callback);
        });
    } else {
        registerListener(table, callback);
    }
};

const randomNumber = (min, max) => {
    const range = max+1 - min;
    return min + Math.floor(range * Math.random());
};

const setActiveCanvas = (id) => {
    compatActiveCanvas = document.getElementById(id);
    compatActiveContext = compatActiveCanvas.getContext('2d');
};

const setFillColor = (color) => {
    compatActiveContext.fillStyle = color;
};

const setStrokeColor = (color) => {
    compatActiveContext.strokeStyle = color;
};

const clearCanvas = (id) => {
    compatActiveContext.clearRect(0, 0, compatActiveCanvas.width, compatActiveCanvas.height);
};

const rect = (x, y, width, height) => {
    compatActiveContext.fillRect(x, y, width, height);
};

const rgb = (r, g, b, a) => {
    a = a === undefined? 1.0 : a;
    return `rgba(${r}, ${g}, ${b}, ${a})`;
};

const appendItem = (array, item) => {
    array.push(item);
};

const timedLoop = (period, callback) => {
    return setInterval(callback, period);
};

const onEvent = (id, kind, callback) => {
    document.getElementById(id).addEventListener(kind, callback);
};

