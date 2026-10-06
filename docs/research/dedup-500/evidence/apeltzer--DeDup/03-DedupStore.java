/*
 * Copyright (c) 2016. DeDup Alexander Peltzer
 * This program is free software: you can redistribute it and/or modify
 *  it under the terms of the GNU General Public License as published by
 *  the Free Software Foundation, either version 3 of the License, or
 *  (at your option) any later version.
 *
 *  This program is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 *  GNU General Public License for more details.
 *
 *  You should have received a copy of the GNU General Public License
 *  along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */

package main.java.datastructure;

import org.apache.commons.lang3.builder.HashCodeBuilder;

/**
 * Created by alex on 2/28/15.
 */
public class DedupStore {
    private int start;
    private int stop;
    private double avgbasequality;

    public DedupStore(int start, int stop) {
        this.start = start;
        this.stop = stop;
    }

    public int getStart() {
        return start;
    }

    public int getStop() {
        return stop;
    }


    @Override
    public boolean equals(Object o) {
        if (!(o instanceof DedupStore)) {
            return false;
        }
        if (((DedupStore) o).getStart() == (this.start)
                && ((DedupStore) o).getStop() == this.stop) {
            return true;
        }
        return false;
    }


    @Override
    public int hashCode() {
        return new HashCodeBuilder(17, 31). // two randomly chosen prime numbers
                // if deriving: appendSuper(super.hashCode()).
                append(this.start).
                append(this.stop).
                toHashCode();

    }
}
